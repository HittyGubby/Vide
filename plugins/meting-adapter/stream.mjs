#!/usr/bin/env node
/* Fetch a URL (with headers) and write the media bytes to stdout. */
const payload = process.argv[2];
if (!payload) {
  process.stderr.write('missing payload\n');
  process.exit(1);
}

let info;
try {
  info = JSON.parse(Buffer.from(payload, 'base64').toString('utf8'));
} catch (error) {
  process.stderr.write('invalid payload: ' + error.message + '\n');
  process.exit(1);
}

try {
  const response = await fetch(info.url, {
    headers: info.headers || {},
    redirect: 'follow',
  });
  if (!response.ok) {
    throw new Error('HTTP ' + response.status);
  }
  if (!response.body) {
    throw new Error('empty response body');
  }
  for await (const chunk of response.body) {
    if (!process.stdout.write(Buffer.from(chunk))) {
      await new Promise((resolve) => process.stdout.once('drain', resolve));
    }
  }
} catch (error) {
  process.stderr.write(String(error.message || error) + '\n');
  process.exitCode = 1;
}
