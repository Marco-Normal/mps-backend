FROM mcr.microsoft.com/playwright/python:v1.49.0-noble
WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt \
    && adduser --disabled-password --gecos "" scraper \
    && chown -R scraper /app
USER scraper
COPY --chown=scraper:scraper . .
CMD ["python", "main.py"]
