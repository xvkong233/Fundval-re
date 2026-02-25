FROM python:3.11-slim

WORKDIR /app

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PORT=8002

COPY pyproject.toml README.md THIRD_PARTY_NOTICES.md /app/
COPY app /app/app
COPY xalpha /app/xalpha

RUN pip install --no-cache-dir -U pip \
  && pip install --no-cache-dir .

EXPOSE 8002

CMD ["sh", "-c", "if [ \"${DEBUG:-false}\" = \"true\" ] || [ \"${DEBUG:-false}\" = \"1\" ]; then exec python -m uvicorn app.main:app --host 0.0.0.0 --port ${PORT:-8002} --log-level debug --access-log; else exec python -m uvicorn app.main:app --host 0.0.0.0 --port ${PORT:-8002} --log-level info --no-access-log; fi"]
