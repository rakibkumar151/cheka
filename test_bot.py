import asyncio
import websockets
import json
import urllib.request
import urllib.error

async def main():
    # 1. Auth to get token
    req = urllib.request.Request(
        "http://127.0.0.1:8080/v1/auth/anonymous",
        data=json.dumps({"user_id": "test_bot_99"}).encode('utf-8'),
        headers={'Content-Type': 'application/json'}
    )
    with urllib.request.urlopen(req) as response:
        res_body = response.read()
        token = json.loads(res_body)["token"]
        print(f"Got token: {token}")

    # 2. Connect to WS
    headers = {"Authorization": f"Bearer {token}"}
    async with websockets.connect("ws://127.0.0.1:8080/v1/ws", extra_headers=headers) as websocket:
        print("Connected as test_bot_99")
        try:
            while True:
                msg = await websocket.recv()
                print(f"< {msg}")
        except websockets.exceptions.ConnectionClosed:
            print("Connection closed")

if __name__ == "__main__":
    asyncio.run(main())
