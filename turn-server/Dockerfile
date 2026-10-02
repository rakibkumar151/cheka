# Use official node image
FROM node:24-slim

WORKDIR /app
COPY turn-server/package*.json ./
RUN npm install

COPY turn-server/index.js ./

# Start TURN server
CMD ["node", "index.js"]
