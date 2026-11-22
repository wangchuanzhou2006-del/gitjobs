//! Private local storage for uploaded tender PDFs.

use std::{
    path::{Path, PathBuf},
    pin::Pin,
};

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
};
use uuid::Uuid;

pub(crate) const MAX_PDF_BYTES: u64 = 50 * 1024 * 1024;
pub(crate) type PdfStream = Pin<Box<dyn AsyncRead + Send>>;

#[derive(Debug, Error)]
pub(crate) enum StorageError {
    #[error("invalid_pdf")]
    InvalidPdf,
    #[error("file_too_large")]
    FileTooLarge,
    #[error("storage_unavailable")]
    Unavailable,
}

pub(crate) struct StoredTenderDocument {
    pub storage_key: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[async_trait]
pub(crate) trait TenderDocumentStore: Send + Sync {
    async fn save(&self, stream: PdfStream) -> Result<StoredTenderDocument, StorageError>;
    async fn open(&self, storage_key: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, storage_key: &str) -> Result<(), StorageError>;
}

pub(crate) struct FileSystemTenderDocumentStore {
    root: PathBuf,
    max_bytes: u64,
}

impl FileSystemTenderDocumentStore {
    pub(crate) async fn new(root: &Path, max_bytes: u64) -> Result<Self, StorageError> {
        if max_bytes == 0 || max_bytes > MAX_PDF_BYTES {
            return Err(StorageError::Unavailable);
        }
        fs::create_dir_all(root)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        if fs::symlink_metadata(root)
            .await
            .map_err(|_| StorageError::Unavailable)?
            .file_type()
            .is_symlink()
        {
            return Err(StorageError::Unavailable);
        }
        let root = fs::canonicalize(root).await.map_err(|_| StorageError::Unavailable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|_| StorageError::Unavailable)?;
        }
        Ok(Self { root, max_bytes })
    }

    async fn existing_path(&self, storage_key: &str) -> Result<PathBuf, StorageError> {
        let id = Uuid::parse_str(storage_key).map_err(|_| StorageError::Unavailable)?;
        if id.to_string() != storage_key {
            return Err(StorageError::Unavailable);
        }
        let path = self.root.join(storage_key);
        let metadata = fs::symlink_metadata(&path)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > self.max_bytes {
            return Err(StorageError::Unavailable);
        }
        let resolved = fs::canonicalize(&path).await.map_err(|_| StorageError::Unavailable)?;
        if resolved.parent() != Some(self.root.as_path()) {
            return Err(StorageError::Unavailable);
        }
        Ok(path)
    }
}

struct RemoveOnDrop(Option<PathBuf>);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[async_trait]
impl TenderDocumentStore for FileSystemTenderDocumentStore {
    async fn save(&self, mut stream: PdfStream) -> Result<StoredTenderDocument, StorageError> {
        let id = Uuid::new_v4().to_string();
        let temporary_path = self.root.join(format!(".{id}.tmp"));
        let final_path = self.root.join(&id);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        // Declare the guard before the handle: on every early return the file
        // closes first, so Windows can remove the partial upload as well.
        let mut cleanup = RemoveOnDrop(Some(temporary_path.clone()));
        let mut file = options
            .open(&temporary_path)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        let mut digest = Sha256::new();
        let mut signature = [0_u8; 5];
        let mut signature_len = 0;
        let mut size = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let count = stream
                .read(&mut buffer)
                .await
                .map_err(|_| StorageError::Unavailable)?;
            if count == 0 {
                break;
            }
            size = size.checked_add(count as u64).ok_or(StorageError::FileTooLarge)?;
            if size > self.max_bytes {
                return Err(StorageError::FileTooLarge);
            }
            let header_bytes = (5 - signature_len).min(count);
            signature[signature_len..signature_len + header_bytes].copy_from_slice(&buffer[..header_bytes]);
            signature_len += header_bytes;
            digest.update(&buffer[..count]);
            file.write_all(&buffer[..count])
                .await
                .map_err(|_| StorageError::Unavailable)?;
        }
        if signature_len != 5 || &signature != b"%PDF-" {
            return Err(StorageError::InvalidPdf);
        }
        file.sync_all().await.map_err(|_| StorageError::Unavailable)?;
        drop(file);
        // Close the temporary file before CPU-bound parsing. The parser owns
        // only bytes, so a cancelled request can still remove the temp file.
        let validation_bytes = fs::read(&temporary_path)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        tokio::task::spawn_blocking(move || {
            let document =
                lopdf::Document::load_mem(&validation_bytes).map_err(|_| StorageError::InvalidPdf)?;
            if document.catalog().is_err() {
                return Err(StorageError::InvalidPdf);
            }
            let pages = document.get_pages();
            if pages.is_empty()
                || pages
                    .values()
                    .any(|page_id| document.get_dictionary(*page_id).is_err())
            {
                return Err(StorageError::InvalidPdf);
            }
            Ok::<(), StorageError>(())
        })
        .await
        .map_err(|_| StorageError::InvalidPdf)??;
        if fs::try_exists(&final_path)
            .await
            .map_err(|_| StorageError::Unavailable)?
        {
            return Err(StorageError::Unavailable);
        }
        fs::rename(&temporary_path, &final_path)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        cleanup.0 = None;
        Ok(StoredTenderDocument {
            storage_key: id,
            sha256: super::hex_bytes(&digest.finalize()),
            size_bytes: size,
        })
    }

    async fn open(&self, storage_key: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.existing_path(storage_key).await?;
        let file = File::open(path).await.map_err(|_| StorageError::Unavailable)?;
        let mut bytes = Vec::new();
        file.take(self.max_bytes + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        if bytes.len() as u64 > self.max_bytes {
            return Err(StorageError::FileTooLarge);
        }
        if !bytes.starts_with(b"%PDF-") {
            return Err(StorageError::InvalidPdf);
        }
        Ok(bytes)
    }

    async fn delete(&self, storage_key: &str) -> Result<(), StorageError> {
        let path = self.existing_path(storage_key).await?;
        fs::remove_file(path).await.map_err(|_| StorageError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root() -> PathBuf {
        std::env::temp_dir().join(format!("tender-store-test-{}", Uuid::new_v4()))
    }

    fn synthetic_pdf(page_count: usize) -> Vec<u8> {
        let mut objects = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            format!(
                "<< /Type /Pages /Kids [{}] /Count {page_count} >>",
                (0..page_count)
                    .map(|index| format!("{} 0 R", index + 3))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .into_bytes(),
        ];
        let contents_id = page_count + 3;
        for _ in 0..page_count {
            objects.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {contents_id} 0 R >>"
                )
                .into_bytes(),
            );
        }
        objects.push(b"<< /Length 0 >>\nstream\n\nendstream".to_vec());

        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            pdf.extend_from_slice(object);
            pdf.extend_from_slice(b"\nendobj\n");
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        pdf
    }

    async fn stream(data: &[u8]) -> PdfStream {
        let (mut writer, reader) = tokio::io::duplex(data.len().max(1));
        writer.write_all(data).await.unwrap();
        drop(writer);
        Box::pin(reader)
    }

    #[tokio::test]
    async fn random_keys_and_roundtrip() {
        let root = temporary_root();
        let store = FileSystemTenderDocumentStore::new(&root, MAX_PDF_BYTES)
            .await
            .unwrap();
        let pdf = synthetic_pdf(1);
        let first = store.save(stream(&pdf).await).await.unwrap();
        let second = store.save(stream(&pdf).await).await.unwrap();
        assert_ne!(first.storage_key, second.storage_key);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.size_bytes, pdf.len() as u64);
        assert_eq!(store.open(&first.storage_key).await.unwrap(), pdf);
        store.delete(&first.storage_key).await.unwrap();
        assert!(store.open(&first.storage_key).await.is_err());
        store.delete(&second.storage_key).await.unwrap();
        fs::remove_dir(root).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_invalid_signature_and_traversal() {
        let root = temporary_root();
        let store = FileSystemTenderDocumentStore::new(&root, MAX_PDF_BYTES)
            .await
            .unwrap();
        assert!(matches!(
            store.save(stream(b"not a pdf").await).await,
            Err(StorageError::InvalidPdf)
        ));
        assert!(matches!(
            store.open("../other").await,
            Err(StorageError::Unavailable)
        ));
        assert!(matches!(
            store.delete("C:\\outside").await,
            Err(StorageError::Unavailable)
        ));
        fs::remove_dir(root).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_pdf_signature_with_broken_structure_without_persisting_it() {
        let root = temporary_root();
        let store = FileSystemTenderDocumentStore::new(&root, MAX_PDF_BYTES)
            .await
            .unwrap();
        assert!(matches!(
            store.save(stream(b"%PDF-fictional").await).await,
            Err(StorageError::InvalidPdf)
        ));
        assert!(
            fs::read_dir(&root)
                .await
                .unwrap()
                .next_entry()
                .await
                .unwrap()
                .is_none()
        );
        fs::remove_dir(root).await.unwrap();
    }

    #[tokio::test]
    async fn enforces_configured_streaming_limit_and_cleans_partial_file() {
        let root = temporary_root();
        let store = FileSystemTenderDocumentStore::new(&root, 8).await.unwrap();
        assert!(matches!(
            store.save(stream(b"%PDF-more-than-eight").await).await,
            Err(StorageError::FileTooLarge)
        ));
        assert!(
            fs::read_dir(&root)
                .await
                .unwrap()
                .next_entry()
                .await
                .unwrap()
                .is_none()
        );
        fs::remove_dir(root).await.unwrap();
    }
}
