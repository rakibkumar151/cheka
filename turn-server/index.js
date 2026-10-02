const Turn = require('node-turn');

const server = new Turn({
  // set listening port
  listeningPort: 3478,
  // set authentication
  authMech: 'long-term',
  credentials: {
    // We will hardcode testuser:testpass for local testing
    "testuser": "testpass"
  }
});

server.start();

console.log("=========================================");
console.log("✅ Local TURN Server started on port 3478");
console.log("Username: testuser");
console.log("Password: testpass");
console.log("URL: turn:10.0.2.2:3478 (from emulator)");
console.log("=========================================");
