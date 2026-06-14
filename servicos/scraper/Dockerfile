FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt \
    && playwright install chromium --with-deps \
    && adduser --disabled-password --gecos "" scraper \
    && chown -R scraper /app
USER scraper
COPY . .
CMD ["python", "main.py"]
