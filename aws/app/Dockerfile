# A typical single-stage Node.js image. Build from the repo root:
#   docker build --platform linux/amd64 -f aws/app/Dockerfile .
FROM node:24

WORKDIR /app
COPY aws/app/package.json aws/app/package-lock.json ./
RUN npm ci

COPY aws/app/tsconfig.json ./
COPY aws/app/src ./src
RUN npm run build

COPY db ./db
ADD https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem /app/rds-ca.pem

ENV NODE_ENV=production PORT=3000 PGSSLROOTCERT=/app/rds-ca.pem
EXPOSE 3000
CMD ["node", "dist/index.js"]
