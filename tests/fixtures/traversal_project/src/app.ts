import { formatLogMessage, computeHash } from './utils';

export interface AppConfig {
  port: number;
  environment: 'development' | 'production' | 'test';
}

export class Application {
  private config: AppConfig;

  constructor(config: AppConfig) {
    this.config = config;
  }

  public start(): void {
    const hash = computeHash(this.config.environment);
    console.log(formatLogMessage(`Application running on port ${this.config.port} [hash: ${hash}]`));
  }
}

export function initializeApplication(): Application {
  return new Application({
    port: 8080,
    environment: 'development',
  });
}
