# Build from the repository root with:
# docker build -f services/tender-analysis/Dockerfile .
FROM python:3.12-slim

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1

RUN groupadd --system tender && useradd --system --gid tender --home-dir /nonexistent tender
WORKDIR /srv/tender-analysis
COPY services/tender-analysis/pyproject.toml ./
COPY services/tender-analysis/app ./app
RUN python -m pip install --no-cache-dir .

USER tender:tender
EXPOSE 8081
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8081", "--no-access-log"]
