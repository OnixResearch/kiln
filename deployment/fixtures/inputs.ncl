# Synthetic evaluation paths, not an inventory or an activation target.
{
  profile_path = "/etc/kiln-web-service/profile.json",
  credential_sources = {
    tls-certificate = "/var/lib/kiln-web-service-credentials/tls-certificate.pem",
    tls-key = "/var/lib/kiln-web-service-credentials/tls-private-key.pem",
    operator-token = "/var/lib/kiln-web-service-credentials/operator-token",
    worker-token = "/var/lib/kiln-web-service-credentials/worker-token",
  },
}
