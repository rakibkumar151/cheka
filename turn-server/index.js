const Turn = require('node-turn');

// Render provides the port in the PORT environment variable
const port = process.env.PORT || 3478;

const server = new Turn({
  // set listening port
  listeningPort: port,
  // set authentication
  authMech: 'long-term',
  credentials: {
    "testuser": "testpass"
  }
});

server.start();

console.log("=========================================");
console.log(`✅ Local TURN Server started on port ${port}`);
console.log("Username: testuser");
console.log("Password: testpass");
console.log("=========================================");
