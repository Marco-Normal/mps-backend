FROM python:3.12-slim
WORKDIR /app

ENV PLAYWRIGHT_BROWSERS_PATH=/usr/local/share/ms-playwright

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt \
    && playwright install chromium --with-deps \
    && adduser --disabled-password --gecos "" scraper \
    && chown -R scraper /app \
    && chmod -R o+rX /usr/local/share/ms-playwright
USER scraper
COPY --chown=scraper:scraper . .
CMD ["python", "main.py"]
