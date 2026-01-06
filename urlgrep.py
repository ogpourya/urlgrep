import sys
import os
import signal

# Clean exit on Ctrl+C during imports
signal.signal(signal.SIGINT, lambda s, f: os._exit(0))

try:
    import argparse
    import asyncio
    import re
    import uvloop
    import aiohttp
except KeyboardInterrupt:
    os._exit(0)

async def fetch_and_scan(session, url, pattern_bytes, timeout_val, retries, proxy, debug):
    for attempt in range(retries + 1):
        try:
            timeout = aiohttp.ClientTimeout(total=timeout_val)
            async with session.get(url, timeout=timeout, proxy=proxy, ssl=False) as response:
                response.raise_for_status()
                
                buffer = bytearray()
                
                async for chunk in response.content.iter_chunked(16384):
                    buffer.extend(chunk)
                    
                    if len(buffer) > 32768:
                        del buffer[:-4096]
                    
                    match = pattern_bytes.search(buffer)
                    if match:
                        print(url)
                        sys.stdout.flush()
                        
                        if debug:
                            m_start = match.start()
                            m_end = match.end()
                            
                            l_start = buffer.rfind(b'\n', 0, m_start) + 1
                            l_end = buffer.find(b'\n', m_end)
                            if l_end == -1: l_end = len(buffer)
                            
                            raw_line = buffer[l_start:l_end]
                            
                            rel_start = m_start - l_start
                            rel_end = m_end - l_start
                            
                            context_len = 50
                            
                            disp_start = max(0, rel_start - (context_len // 2))
                            disp_end = min(len(raw_line), disp_start + context_len)
                            
                            if disp_end - disp_start < context_len:
                                disp_start = max(0, disp_end - context_len)
                            
                            display_bytes = raw_line[disp_start:disp_end]
                            
                            d_rel_start = max(0, rel_start - disp_start)
                            d_rel_end = min(len(display_bytes), rel_end - disp_start)
                            
                            pre = display_bytes[:d_rel_start].decode('utf-8', 'replace').replace('\n', ' ')
                            match_txt = display_bytes[d_rel_start:d_rel_end].decode('utf-8', 'replace').replace('\n', ' ')
                            post = display_bytes[d_rel_end:].decode('utf-8', 'replace').replace('\n', ' ')
                            
                            sys.stderr.write(f"Found: {pre}\033[91m{match_txt}\033[0m{post}\n")
                            sys.stderr.flush()
                        return
            return
        except Exception:
            if attempt == retries:
                return
            await asyncio.sleep(0.1 * (attempt + 1))

async def worker(queue, session, pattern, timeout, retries, proxy, debug):
    while True:
        url = await queue.get()
        if url is None:
            queue.task_done()
            break
        
        await fetch_and_scan(session, url, pattern, timeout, retries, proxy, debug)
        queue.task_done()

async def reader(queue, loop):
    reader_stream = asyncio.StreamReader()
    protocol = asyncio.StreamReaderProtocol(reader_stream)
    await loop.connect_read_pipe(lambda: protocol, sys.stdin)
    
    async for line in reader_stream:
        line = line.decode('utf-8').strip()
        if not line:
            continue
            
        if not line.startswith(('http://', 'https://')):
            line = 'https://' + line
            
        await queue.put(line)

async def main():
    parser = argparse.ArgumentParser(description="Async URL grep")
    parser.add_argument("pattern", help="Regex pattern")
    parser.add_argument("-t", "--timeout", type=float, default=3.0, help="Timeout in seconds")
    parser.add_argument("-p", "--proxy", type=str, default=None, help="Proxy URL")
    parser.add_argument("-w", "--workers", type=int, default=10, help="Concurrency limit")
    parser.add_argument("-r", "--retry", type=int, default=3, help="Max retries")
    parser.add_argument("-d", "--debug", action="store_true", help="Enable debug output")
    
    args = parser.parse_args()
    
    try:
        regex = re.compile(args.pattern.encode('utf-8'))
    except re.error:
        sys.exit(1)

    queue = asyncio.Queue(maxsize=args.workers * 2)
    
    connector = aiohttp.TCPConnector(limit=0, ttl_dns_cache=300, use_dns_cache=True)
    async with aiohttp.ClientSession(connector=connector, cookie_jar=aiohttp.DummyCookieJar()) as session:
        workers = [
            asyncio.create_task(worker(queue, session, regex, args.timeout, args.retry, args.proxy, args.debug))
            for _ in range(args.workers)
        ]
        
        loop = asyncio.get_running_loop()
        reader_task = asyncio.create_task(reader(queue, loop))
        
        try:
            await reader_task
            for _ in range(args.workers):
                await queue.put(None)
            await queue.join()
            await asyncio.gather(*workers)
        except asyncio.CancelledError:
            pass

if __name__ == "__main__":
    uvloop.install()
    signal.signal(signal.SIGINT, lambda s, f: os._exit(0))
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        os._exit(0)
