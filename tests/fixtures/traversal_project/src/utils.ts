export function formatLogMessage(msg: string): string {
  const timestamp = new Date().toISOString();
  return `[${timestamp}] ${msg}`;
}

export function computeHash(input: string): string {
  let hash = 0;
  for (let i = 0; i < input.length; i++) {
    hash = (hash << 5) - hash + input.charCodeAt(i);
    hash |= 0;
  }
  return hash.toString(16);
}
