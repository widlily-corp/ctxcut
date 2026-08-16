"""
Circular and self-referencing data models using modern typing and Pydantic.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional
from pydantic import BaseModel, Field


class CategoryNode(BaseModel):
    id: str
    name: str
    parent: Optional[CategoryNode] = None
    subcategories: list[CategoryNode] = Field(default_factory=list)


class GraphNodeModel(BaseModel):
    id: str
    label: str
    neighbors: list[GraphNodeModel] = Field(default_factory=list)
    edges: list[GraphEdgeModel] = Field(default_factory=list)


class GraphEdgeModel(BaseModel):
    id: str
    source: GraphNodeModel
    target: GraphNodeModel
    weight: float = 1.0


@dataclass
class OrganizationUnit:
    name: str
    headcount: int
    parent_unit: Optional[OrganizationUnit] = None
    subunits: list[OrganizationUnit] = field(default_factory=list)

    def total_subordinate_count(self) -> int:
        count = self.headcount
        for sub in self.subunits:
            count += sub.total_subordinate_count()
        return count


@dataclass
class SyntaxTreeNode:
    kind: str
    value: Optional[str] = None
    parent: Optional[SyntaxTreeNode] = None
    children: list[SyntaxTreeNode] = field(default_factory=list)


def build_taxonomy_tree() -> CategoryNode:
    root = CategoryNode(id="cat_0", name="Electronics")
    computers = CategoryNode(id="cat_1", name="Computers", parent=root)
    laptops = CategoryNode(id="cat_2", name="Laptops", parent=computers)
    
    computers.subcategories.append(laptops)
    root.subcategories.append(computers)
    return root
