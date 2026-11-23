"""An aiocoap client for crates/coap-bridge/tests/cross_vendor.rs
(zerodds-coap-bridge-1.0 §10): `post URI JSON`, `get URI` and
`observe URI SECONDS`, the payloads as JSON (Content-Format 50), every
response and notification as a line `<code> <payload>` on stdout."""

import asyncio
import sys

from aiocoap import GET, POST, Context, Message


def show(message):
    code = int(message.code)
    text = message.payload.decode("utf-8", errors="replace")
    print(f"{code >> 5}.{code & 0x1F:02d} {text}", flush=True)


async def main(mode, uri, argument=None):
    context = await Context.create_client_context()
    if mode == "post":
        request = Message(code=POST, uri=uri, payload=argument.encode())
        request.opt.content_format = 50
        show(await context.request(request).response)
    elif mode == "get":
        request = Message(code=GET, uri=uri)
        request.opt.accept = 50
        show(await context.request(request).response)
    elif mode == "observe":
        request = Message(code=GET, uri=uri, observe=0)
        request.opt.accept = 50
        pending = context.request(request)
        show(await pending.response)
        try:
            async with asyncio.timeout(float(argument)):
                async for notification in pending.observation:
                    show(notification)
        except TimeoutError:
            pass
        pending.observation.cancel()
    else:
        print("post URI JSON | get URI | observe URI SECONDS", file=sys.stderr)
        sys.exit(2)
    await context.shutdown()


if __name__ == "__main__":
    asyncio.run(main(*sys.argv[1:]))
