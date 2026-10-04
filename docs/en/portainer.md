# Running with Portainer

`docker-compose.yml` only uses prebuilt images from GHCR and can therefore be
used directly as a Portainer stack.

1. **Stacks → Add stack**, name `talkops`.
2. Build method **Repository** (repository URL
   `https://github.com/itsh-neumeier/talkops`, compose path `docker-compose.yml`)
   or **Web editor** and paste the contents of `docker-compose.yml`.
3. Under **Environment variables** → **Advanced mode** paste the contents of
   `.env.example` and fill in the required values (`POSTGRES_PASSWORD`,
   `TALKOPS_SECRET_KEY`, `TALKOPS_ESL_PASSWORD`, `TALKOPS_XMLCURL_PASSWORD`).
   Generate random values e.g. with `openssl rand -hex 32`.
4. **Deploy the stack**. After about a minute all containers should be healthy.

Notes:

- The Portainer endpoint must be a Linux Docker host (host networking).
- Updates: open the stack → **Pull and redeploy** (for repository stacks,
  enable "Re-pull image").
- Back up `TALKOPS_SECRET_KEY` outside of Portainer as well.
