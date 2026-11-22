const BUDGET = "Budget amount: CNY 2800000";
const DEADLINE = "Bid deadline: 2026-09-01";

// A genuine two-page PDF with uncompressed ASCII text for the acceptance mock.
export function syntheticTenderPdf(forged = false, blank = false) {
  const pageStream = (line) => `BT /F1 12 Tf 50 700 Td (${line}) Tj ET\n`;
  const first = pageStream(blank ? "" : BUDGET);
  const second = pageStream(
    blank ? "" : forged ? `${DEADLINE} E2E_FORGED_BID` : DEADLINE,
  );
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R >>",
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    `<< /Length ${Buffer.byteLength(first)} >>\nstream\n${first}endstream`,
    `<< /Length ${Buffer.byteLength(second)} >>\nstream\n${second}endstream`,
  ];
  let pdf = `%PDF-1.4\n% synthetic-${Date.now()}-${Math.random()}\n`;
  const offsets = [0];
  for (const [index, object] of objects.entries()) {
    offsets.push(Buffer.byteLength(pdf));
    pdf += `${index + 1} 0 obj\n${object}\nendobj\n`;
  }
  const xref = Buffer.byteLength(pdf);
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets.slice(1))
    pdf += `${String(offset).padStart(10, "0")} 00000 n \n`;
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf, "ascii");
}
