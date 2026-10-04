# Betrieb mit Portainer

`docker-compose.yml` verwendet ausschließlich fertige Images von GHCR und ist
daher direkt als Portainer-Stack nutzbar.

1. **Stacks → Add stack**, Name `talkops`.
2. Build-Methode **Repository** (Repository-URL
   `https://github.com/itsh-neumeier/talkops`, Compose-Pfad `docker-compose.yml`)
   oder **Web editor** und den Inhalt von `docker-compose.yml` einfügen.
3. Unter **Environment variables** → **Advanced mode** den Inhalt von
   `.env.example` einfügen und die Pflichtwerte setzen (`POSTGRES_PASSWORD`,
   `TALKOPS_SECRET_KEY`, `TALKOPS_ESL_PASSWORD`, `TALKOPS_XMLCURL_PASSWORD`).
   Zufallswerte z. B. mit `openssl rand -hex 32` erzeugen.
4. **Deploy the stack**. Nach ca. einer Minute sollten alle Container
   „healthy“ sein.

Hinweise:

- Der Portainer-Endpoint muss ein Linux-Docker-Host sein (Host-Networking).
- Updates: Stack öffnen → **Pull and redeploy** (bei Repository-Stacks
  „Re-pull image“ aktivieren).
- `TALKOPS_SECRET_KEY` zusätzlich außerhalb von Portainer sichern.
