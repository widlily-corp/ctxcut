/**
 * TypeScript class indented with tabs.
 */

export class TabIndentedBuffer {
	private buffer: string[];

	constructor() {
		this.buffer = [];
	}

	public append(item: string): void {
		this.buffer.push(item);
	}

	public flush(): string {
		const result = this.buffer.join("\n");
		this.buffer = [];
		return result;
	}
}
