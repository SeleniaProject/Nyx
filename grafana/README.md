# Nyx Grafana Stack (Prometheus + Tempo)

## Quick start

1. Start the observability stack:
   - `docker compose -f docker-compose.grafana.yml up -d`
2. Run nyx-daemon with metrics:
   - Set `NYX_PROMETHEUS_ADDR=0.0.0.0:9100` before starting the process
3. (Optional) Enable OTLP tracing to Tempo:
   - Set `NYX_OTLP=1`
   - Ensure `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://localhost:4317`
4. Open Grafana: http://localhost:3000 (admin/admin)
   - Dashboards are auto-provisioned from `grafana/dashboards`.

## Notes
- Prometheus scrapes `host.docker.internal:9100` by default. Adjust if running on Linux.
- Tempo receives OTLP gRPC on 4317.