/**
 * TypeScript class indented with 2 spaces.
 */

export class TwoSpaceFormatter {
  private prefix: string;

  constructor(prefix: string = '[LOG]') {
    this.prefix = prefix;
  }

  public formatMessage(message: string): string {
    const timestamp = new Date().toISOString();
    return `${this.prefix} ${timestamp}: ${message}`;
  }

  public setPrefix(newPrefix: string): void {
    this.prefix = newPrefix;
  }
}
