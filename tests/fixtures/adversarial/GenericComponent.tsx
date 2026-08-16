import React from 'react';

export interface RowItem {
    id: string | number;
    title: string;
    score?: number;
}

export interface TableProps<T extends RowItem> {
    items: T[];
    onRowSelect?: (item: T) => void;
    renderCustomCell?: (item: T) => React.ReactNode;
}

export function useTableSort<T>(items: T[]): T[] {
    return items.slice();
}

/**
 * Generic Table TSX component.
 */
export const GenericTable = <T extends RowItem,>(props: TableProps<T>): React.ReactElement => {
    const sorted = useTableSort(props.items);

    return (
        <div className="table-container">
            <header className="table-header">
                <h2>Total Items: {sorted.length}</h2>
            </header>
            <main className="table-body">
                {sorted.map((item) => (
                    <article
                        key={item.id}
                        className="table-row"
                        onClick={() => props.onRowSelect && props.onRowSelect(item)}
                    >
                        <span>{item.title}</span>
                        {props.renderCustomCell ? (
                            props.renderCustomCell(item)
                        ) : (
                            <small>{item.score ?? 0}</small>
                        )}
                    </article>
                ))}
            </main>
        </div>
    );
};
