import { initializeApplication } from './app';

export function main(): void {
  const app = initializeApplication();
  app.start();
}

if (require.main === module) {
  main();
}
