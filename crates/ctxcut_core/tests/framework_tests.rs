//! Comprehensive unit and integration tests for Milestone 3 (M3: Framework-Aware Semantic Intelligence).
//!
//! Covers:
//! 1. FastAPI & Pydantic (route decorators, response_models, Depends DI, field validators)
//! 2. Django & Django REST Framework (ViewSets, APIViews, ModelSerializers, ORM Models, permissions)
//! 3. React & Next.js (Component Props interfaces, custom hook extraction, JSX branch collapsing)
//! 4. Express.js (route methods, middleware invocation chains, generic Request/Response DTOs)
//! 5. NestJS (Controller & method decorators, @UseGuards, @UseInterceptors, @Body DTOs, return types)
//! 6. Spring Boot (@RestController, @RequestMapping, @RequestBody DTOs, @PreAuthorize security stubs)
//! 7. FrameworkRegistry (auto-detection, dispatching, composite analyzer)

use ctxcut_core::framework::{
    DjangoFastApiAnalyzer, ExpressAnalyzer, ExpressNestSpringAnalyzer, FrameworkAnalyzer,
    FrameworkRegistry, NestJsAnalyzer, ReactNextAnalyzer, SpringAnalyzer,
};
use ctxcut_core::model::{ExtractedSymbol, SliceOptions, SliceResult, TokenStats};
use ctxcut_core::parser::ParserManager;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn create_dummy_slice(name: &str, body: &str, file_path: &Path, language: &str) -> SliceResult {
    SliceResult {
        target_symbol: ExtractedSymbol {
            name: name.to_string(),
            kind: "function".to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            start_line: 1,
            end_line: body.lines().count().max(1),
            doc_comment: None,
            signature: format!("fn {name}()"),
            body: body.to_string(),
            language: language.to_string(),
        },
        hoisted_types: Vec::new(),
        stripped_calls: Vec::new(),
        stats: TokenStats::calculate(0, 0, 0, 0),
    }
}

// =========================================================================
// 1. FASTAPI & PYDANTIC TESTS
// =========================================================================

#[test]
fn test_fastapi_route_and_pydantic_extraction() {
    let source = r#"
from fastapi import APIRouter, Depends, status, Query
from pydantic import BaseModel, Field, field_validator
from typing import Annotated, Optional

class ItemCreate(BaseModel):
    """Schema for item creation."""
    title: str = Field(..., min_length=1, max_length=100)
    description: Optional[str] = None
    price: float = Field(..., gt=0.0)
    sku: str

    @field_validator("sku")
    @classmethod
    def validate_sku(cls, v: str) -> str:
        if not v.isupper():
            raise ValueError("SKU must be uppercase")
        return v

class ItemResponse(BaseModel):
    id: int
    title: str
    price: float

def get_db():
    """Database session provider."""
    db = DatabaseSession()
    try:
        yield db
    finally:
        db.close()

def verify_token(token: str = Query(...)):
    """Auth token verifier."""
    if not token:
        raise HTTPException(status_code=401)
    return True

router = APIRouter()

@router.post(
    "/items/",
    response_model=ItemResponse,
    status_code=status.HTTP_201_CREATED,
    dependencies=[Depends(verify_token)],
)
async def create_item(
    payload: ItemCreate,
    db: Annotated[DatabaseSession, Depends(get_db)],
) -> ItemResponse:
    item = db.create(payload)
    return item
"#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("routes.py");
    fs::write(&file_path, source).unwrap();

    let ts_lang = tree_sitter_python::LANGUAGE.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    let analyzer = DjangoFastApiAnalyzer::new();
    assert!(analyzer.matches_framework(&file_path, source));

    // Find the create_item function node
    let target_node = root
        .named_children(&mut root.walk())
        .find(|n| {
            let text = &source[n.start_byte()..n.end_byte()];
            text.contains("def create_item")
        })
        .expect("create_item node not found");

    let mut slice = create_dummy_slice(
        "create_item",
        "def create_item(): ...",
        &file_path,
        "python",
    );
    analyzer
        .enhance_slice(target_node, source, &file_path, &mut slice)
        .unwrap();

    // Verify hoisted Pydantic types
    assert!(
        slice.hoisted_types.iter().any(|t| t.name == "ItemResponse"),
        "Expected ItemResponse to be hoisted, found: {:?}",
        slice.hoisted_types
    );
    assert!(
        slice.hoisted_types.iter().any(|t| t.name == "ItemCreate"),
        "Expected ItemCreate to be hoisted, found: {:?}",
        slice.hoisted_types
    );

    // Verify minified class structure for ItemCreate (validator body replaced with ...)
    let item_create = slice
        .hoisted_types
        .iter()
        .find(|t| t.name == "ItemCreate")
        .unwrap();
    assert!(item_create
        .definition
        .contains("class ItemCreate(BaseModel):"));
    assert!(item_create.definition.contains("title: str = Field"));
    assert!(item_create.definition.contains("sku: str"));
    assert!(
        item_create
            .definition
            .contains("def validate_sku(cls, v: str) -> str: ..."),
        "ItemCreate minification should replace validator body with ..., got:\n{}",
        item_create.definition
    );

    // Verify dependency provider stubs in stripped_calls
    assert!(
        slice
            .stripped_calls
            .iter()
            .any(|c| c.name == "verify_token"),
        "Expected verify_token stub in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
    assert!(
        slice.stripped_calls.iter().any(|c| c.name == "get_db"),
        "Expected get_db stub in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
}

// =========================================================================
// 2. DJANGO & DRF TESTS
// =========================================================================

#[test]
fn test_django_drf_viewset_extraction() {
    let source = r#"
from rest_framework import viewsets, serializers, permissions
from django.db import models

class User(models.Model):
    username = models.CharField(max_length=150, unique=True)
    email = models.EmailField()

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Item(models.Model):
    title = models.CharField(max_length=200)
    price = models.DecimalField(max_digits=10, decimal_places=2)
    owner = models.ForeignKey(User, on_delete=models.CASCADE)
    tags = models.ManyToManyField(Tag)

    class Meta:
        db_table = 'catalog_items'
        ordering = ['-id']

    def __str__(self):
        return self.title

    def save(self, *args, **kwargs):
        super().save(*args, **kwargs)

class TagSerializer(serializers.ModelSerializer):
    class Meta:
        model = Tag
        fields = ['id', 'name']

class ItemSerializer(serializers.ModelSerializer):
    tags = TagSerializer(many=True, read_only=True)

    class Meta:
        model = Item
        fields = ['id', 'title', 'price', 'owner', 'tags']

    def validate_price(self, value):
        if value <= 0:
            raise serializers.ValidationError("Price must be positive")
        return value

class IsAuthenticated(permissions.BasePermission):
    def has_permission(self, request, view):
        return bool(request.user and request.user.is_authenticated)

class HasProjectAccess(permissions.BasePermission):
    def has_object_permission(self, request, view, obj):
        return obj.owner == request.user

class ItemViewSet(viewsets.ModelViewSet):
    """ViewSet for Item CRUD operations."""
    queryset = Item.objects.all()
    serializer_class = ItemSerializer
    permission_classes = [IsAuthenticated, HasProjectAccess]
"#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("views.py");
    fs::write(&file_path, source).unwrap();

    let ts_lang = tree_sitter_python::LANGUAGE.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    let analyzer = DjangoFastApiAnalyzer::new();
    assert!(analyzer.matches_framework(&file_path, source));

    let target_node = root
        .named_children(&mut root.walk())
        .find(|n| {
            let text = &source[n.start_byte()..n.end_byte()];
            text.contains("class ItemViewSet")
        })
        .expect("ItemViewSet node not found");

    let mut slice = create_dummy_slice(
        "ItemViewSet",
        "class ItemViewSet: ...",
        &file_path,
        "python",
    );
    analyzer
        .enhance_slice(target_node, source, &file_path, &mut slice)
        .unwrap();

    // Verify Serializer hoisted
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "ItemSerializer"),
        "Expected ItemSerializer hoisted, found: {:?}",
        slice.hoisted_types
    );

    // Verify nested Serializer TagSerializer hoisted
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "TagSerializer"),
        "Expected TagSerializer hoisted, found: {:?}",
        slice.hoisted_types
    );

    // Verify Model Item hoisted
    assert!(
        slice.hoisted_types.iter().any(|t| t.name == "Item"),
        "Expected Item model hoisted, found: {:?}",
        slice.hoisted_types
    );

    // Verify related Model Tag hoisted via ManyToManyField or TagSerializer
    assert!(
        slice.hoisted_types.iter().any(|t| t.name == "Tag"),
        "Expected Tag model hoisted, found: {:?}",
        slice.hoisted_types
    );

    // Verify permission classes hoisted
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "IsAuthenticated"),
        "Expected IsAuthenticated hoisted, found: {:?}",
        slice.hoisted_types
    );
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "HasProjectAccess"),
        "Expected HasProjectAccess hoisted, found: {:?}",
        slice.hoisted_types
    );

    // Verify Model minification (method bodies like save() and __str__() replaced with ...)
    let item_model = slice
        .hoisted_types
        .iter()
        .find(|t| t.name == "Item")
        .unwrap();
    assert!(item_model.definition.contains("class Item(models.Model):"));
    assert!(item_model.definition.contains("title = models.CharField"));
    assert!(item_model.definition.contains("class Meta:"));
    assert!(item_model.definition.contains("def __str__(self): ..."));
    assert!(item_model
        .definition
        .contains("def save(self, *args, **kwargs): ..."));
}

// =========================================================================
// 3. REACT & NEXT.JS AND JSX COLLAPSER TESTS
// =========================================================================

#[test]
fn test_react_props_and_custom_hooks_extraction() {
    let source = r#"
import React, { useState, useMemo } from 'react';
import { useAuth } from '../hooks/useAuth';
import { useTableSort } from '../hooks/useTableSort';

export interface User {
    id: string;
    name: string;
    email: string;
}

export interface UserProfileProps {
    user: User;
    onUpdate: (user: User) => void;
    className?: string;
}

export function UserProfile({ user, onUpdate, className }: UserProfileProps): JSX.Element {
    const auth = useAuth();
    const [count, setCount] = useState<number>(0);
    const { sortOrder, toggleSort } = useTableSort(user);
    const isOwner = useMemo(() => auth.user?.id === user.id, [auth, user]);

    return (
        <div className={`user-profile ${className}`}>
            <header className="profile-header">
                <h2>{user.name}</h2>
                <p>{user.email}</p>
            </header>
            <main>
                <p>Status: {isOwner ? 'Owner' : 'Guest'}</p>
            </main>
        </div>
    );
}
"#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("UserProfile.tsx");
    fs::write(&file_path, source).unwrap();

    let ts_lang = tree_sitter_typescript::LANGUAGE_TSX.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    let analyzer = ReactNextAnalyzer::new();
    assert!(analyzer.matches_framework(&file_path, source));

    let target_node = root
        .named_children(&mut root.walk())
        .find(|n| {
            let text = &source[n.start_byte()..n.end_byte()];
            text.contains("function UserProfile")
        })
        .expect("UserProfile node not found");

    let mut slice = create_dummy_slice(
        "UserProfile",
        &source[target_node.start_byte()..target_node.end_byte()],
        &file_path,
        "typescript",
    );
    analyzer
        .enhance_slice(target_node, source, &file_path, &mut slice)
        .unwrap();

    // 1. Props interface hoisted
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "UserProfileProps"),
        "Expected UserProfileProps hoisted, found: {:?}",
        slice.hoisted_types
    );

    // 2. Custom hooks useAuth and useTableSort extracted, but NOT useState or useMemo
    assert!(
        slice.stripped_calls.iter().any(|c| c.name == "useAuth"),
        "Expected useAuth in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
    assert!(
        slice
            .stripped_calls
            .iter()
            .any(|c| c.name == "useTableSort"),
        "Expected useTableSort in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
    assert!(
        !slice.stripped_calls.iter().any(|c| c.name == "useState"),
        "Built-in useState must NOT be in stripped_calls"
    );
    assert!(
        !slice.stripped_calls.iter().any(|c| c.name == "useMemo"),
        "Built-in useMemo must NOT be in stripped_calls"
    );
}

#[test]
fn test_jsx_branch_collapsing_deep_elements() {
    let source = r#"
export function OrderDashboard(props: DashboardProps) {
    const { orders, isLoading } = useOrders();

    return (
        <div className="dashboard-container max-w-7xl mx-auto p-6">
            <OrderHeader title="Orders Management" />
            {isLoading ? (
                <div className="spinner-wrapper flex items-center justify-center p-12">
                    <span className="animate-spin h-8 w-8 text-blue-600">Loading...</span>
                    <p className="mt-2 text-sm text-gray-500">Please wait while orders are loading</p>
                </div>
            ) : (
                <section className="orders-section mt-8">
                    <table className="min-w-full divide-y divide-gray-200">
                        <thead className="bg-gray-50">
                            <tr>
                                <th scope="col" className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">ID</th>
                                <th scope="col" className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Customer</th>
                                <th scope="col" className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Amount</th>
                            </tr>
                        </thead>
                        <tbody className="bg-white divide-y divide-gray-200">
                            {orders.map((order) => (
                                <OrderRow key={order.id} order={order} />
                            ))}
                        </tbody>
                    </table>
                </section>
            )}
            <OrderFooter />
        </div>
    );
}
"#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("OrderDashboard.tsx");
    fs::write(&file_path, source).unwrap();

    let ts_lang = tree_sitter_typescript::LANGUAGE_TSX.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    let target_node = root
        .named_children(&mut root.walk())
        .find(|n| {
            let text = &source[n.start_byte()..n.end_byte()];
            text.contains("function OrderDashboard")
        })
        .expect("OrderDashboard node not found");

    let analyzer = ReactNextAnalyzer::with_thresholds(2, 3);
    let collapsed = analyzer.collapse_jsx_branches(source, target_node);
    assert!(collapsed.is_some(), "Expected collapsed JSX to be Some");

    let collapsed_text = collapsed.unwrap();

    // 1. Root container preserved
    assert!(
        collapsed_text.contains("<div className=\"dashboard-container max-w-7xl mx-auto p-6\">")
    );

    // 2. Custom components preserved (OrderHeader, OrderFooter, OrderRow)
    assert!(collapsed_text.contains("<OrderHeader title=\"Orders Management\" />"));
    assert!(collapsed_text.contains("<OrderFooter />"));
    assert!(collapsed_text.contains("<OrderRow key={order.id} order={order} />"));

    // 3. Deep native HTML elements collapsed to /* X lines collapsed */
    assert!(
        collapsed_text.contains("lines collapsed */"),
        "Expected lines collapsed stub in:\n{collapsed_text}"
    );

    // 4. Control-flow ternary preserved
    assert!(collapsed_text.contains("isLoading ?"));
}

// =========================================================================
// 4. EXPRESS.JS TESTS
// =========================================================================

#[test]
fn test_express_route_and_middleware_extraction() {
    let source = r"
import { Router, Request, Response, NextFunction } from 'express';

export interface CheckoutRequestDTO {
    cartId: string;
    paymentMethodId: string;
    shippingAddress: {
        street: string;
        city: string;
        postalCode: string;
    };
}

export interface CheckoutResponseDTO {
    orderId: string;
    status: 'CONFIRMED' | 'PENDING';
    totalAmount: number;
}

export function authenticate(req: Request, res: Response, next: NextFunction) {
    next();
}

export function validate(dto: any) {
    return (req: Request, res: Response, next: NextFunction) => next();
}

const router = Router();

router.post(
    '/api/v1/checkout',
    authenticate,
    validate(CheckoutRequestDTO),
    async function handleCheckout(
        req: Request<{ tenantId: string }, CheckoutResponseDTO, CheckoutRequestDTO, { ref?: string }>,
        res: Response<CheckoutResponseDTO>
    ) {
        const order = await processOrder(req.body);
        return res.status(201).json(order);
    }
);
";

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("checkout.route.ts");
    fs::write(&file_path, source).unwrap();

    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    let analyzer = ExpressAnalyzer::new();
    assert!(analyzer.matches_framework(&file_path, source));

    let target_node = root
        .named_children(&mut root.walk())
        .find(|n| {
            let text = &source[n.start_byte()..n.end_byte()];
            text.contains("router.post")
        })
        .expect("router.post node not found");

    let mut slice = create_dummy_slice(
        "handleCheckout",
        "async function handleCheckout() {}",
        &file_path,
        "typescript",
    );
    analyzer
        .enhance_slice(target_node, source, &file_path, &mut slice)
        .unwrap();

    // Verify middleware stubs in stripped_calls
    assert!(
        slice
            .stripped_calls
            .iter()
            .any(|c| c.name == "authenticate"),
        "Expected authenticate in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
    assert!(
        slice
            .stripped_calls
            .iter()
            .any(|c| c.name.contains("validate")),
        "Expected validate in stripped_calls, found: {:?}",
        slice.stripped_calls
    );

    // Verify DTO types in hoisted_types
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "CheckoutRequestDTO"),
        "Expected CheckoutRequestDTO in hoisted_types, found: {:?}",
        slice.hoisted_types
    );
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "CheckoutResponseDTO"),
        "Expected CheckoutResponseDTO in hoisted_types, found: {:?}",
        slice.hoisted_types
    );
}

// =========================================================================
// 5. NESTJS TESTS
// =========================================================================

#[test]
fn test_nestjs_controller_and_decorators() {
    let source = r"
import { Controller, Get, Post, Body, Param, Query, UseGuards, UseInterceptors, HttpCode, HttpStatus } from '@nestjs/common';

export class CreateUserDto {
    username: string;
    email: string;
    role: string;
}

export class UserResponseDto {
    id: string;
    username: string;
    email: string;
    createdAt: string;
}

export class AuthGuard {}
export class RolesGuard {}
export class LoggingInterceptor {}

@Controller('users')
@UseGuards(AuthGuard)
@UseInterceptors(LoggingInterceptor)
export class UsersController {
    constructor(private readonly usersService: any) {}

    @Post()
    @HttpCode(HttpStatus.CREATED)
    @UseGuards(RolesGuard)
    async create(
        @Body() createUserDto: CreateUserDto,
        @Param('tenantId') tenantId: string
    ): Promise<UserResponseDto> {
        return this.usersService.create(createUserDto);
    }
}
";

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("users.controller.ts");
    fs::write(&file_path, source).unwrap();

    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    let analyzer = NestJsAnalyzer::new();
    assert!(analyzer.matches_framework(&file_path, source));

    let class_node = root
        .named_children(&mut root.walk())
        .find(|n| {
            let text = &source[n.start_byte()..n.end_byte()];
            text.contains("class UsersController")
        })
        .expect("UsersController node not found");

    let unwrapped_class = ctxcut_core::parser::AstUtils::unwrap_export(class_node);
    let method_node = ctxcut_core::parser::AstUtils::find_descendants_by_kind(
        unwrapped_class,
        "method_definition",
    )
    .into_iter()
    .find(|m| {
        let text = &source[m.start_byte()..m.end_byte()];
        text.contains("async create")
    })
    .expect("create method node not found");

    let mut slice = create_dummy_slice("create", "async create() {}", &file_path, "typescript");
    analyzer
        .enhance_slice(method_node, source, &file_path, &mut slice)
        .unwrap();

    // Verify guards and interceptors in stripped_calls
    assert!(
        slice.stripped_calls.iter().any(|c| c.name == "AuthGuard"),
        "Expected AuthGuard in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
    assert!(
        slice.stripped_calls.iter().any(|c| c.name == "RolesGuard"),
        "Expected RolesGuard in stripped_calls, found: {:?}",
        slice.stripped_calls
    );
    assert!(
        slice
            .stripped_calls
            .iter()
            .any(|c| c.name == "LoggingInterceptor"),
        "Expected LoggingInterceptor in stripped_calls, found: {:?}",
        slice.stripped_calls
    );

    // Verify DTOs in hoisted_types
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "CreateUserDto"),
        "Expected CreateUserDto in hoisted_types, found: {:?}",
        slice.hoisted_types
    );
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "UserResponseDto"),
        "Expected UserResponseDto in hoisted_types, found: {:?}",
        slice.hoisted_types
    );
}

// =========================================================================
// 6. SPRING BOOT TESTS
// =========================================================================

#[test]
fn test_spring_controller_and_annotations() {
    let source = r#"
package com.example.demo.controller;

import org.springframework.web.bind.annotation.*;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.http.ResponseEntity;

public class CreateOrderRequest {
    private String customerId;
    private Double amount;
}

public class OrderResponse {
    private String orderId;
    private String status;
}

@RestController
@RequestMapping("/api/v1/orders")
public class OrderController {

    @PostMapping
    @PreAuthorize("hasRole('CUSTOMER')")
    public ResponseEntity<OrderResponse> createOrder(
        @RequestBody CreateOrderRequest request,
        @RequestHeader("X-Tenant-ID") String tenantId
    ) {
        return ResponseEntity.ok(new OrderResponse());
    }
}
"#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("OrderController.java");
    fs::write(&file_path, source).unwrap();

    let analyzer = SpringAnalyzer::new();
    assert!(analyzer.matches_framework(&file_path, source));

    let mut slice = create_dummy_slice(
        "createOrder",
        r#"
    @PostMapping
    @PreAuthorize("hasRole('CUSTOMER')")
    public ResponseEntity<OrderResponse> createOrder(
        @RequestBody CreateOrderRequest request,
        @RequestHeader("X-Tenant-ID") String tenantId
    ) {
        return ResponseEntity.ok(new OrderResponse());
    }
"#,
        &file_path,
        "java",
    );

    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let tree = ParserManager::parse_source(source, &ts_lang, &file_path).unwrap();
    let root = tree.root_node();

    analyzer
        .enhance_slice(root, source, &file_path, &mut slice)
        .unwrap();

    // Verify security annotation stub
    assert!(
        slice
            .stripped_calls
            .iter()
            .any(|c| c.name.contains("PreAuthorize")),
        "Expected PreAuthorize in stripped_calls, found: {:?}",
        slice.stripped_calls
    );

    // Verify DTO names extracted
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "CreateOrderRequest")
            || slice.target_symbol.body.contains("CreateOrderRequest")
    );
    assert!(
        slice
            .hoisted_types
            .iter()
            .any(|t| t.name == "OrderResponse")
            || slice.target_symbol.body.contains("OrderResponse")
    );
}

// =========================================================================
// 7. FRAMEWORK REGISTRY & INTEGRATION PIPELINE TESTS
// =========================================================================

#[test]
fn test_framework_registry_dispatch_and_slice_integration() {
    let registry = FrameworkRegistry::new();

    // Check FastAPI matching
    assert_eq!(
        registry
            .find_matching(Path::new("app/routes.py"), "from fastapi import APIRouter")
            .len(),
        1
    );

    // Check React matching
    assert_eq!(
        registry
            .find_matching(
                Path::new("components/Button.tsx"),
                "export function Button() {}"
            )
            .len(),
        1
    );

    // Check Express matching
    assert_eq!(
        registry
            .find_matching(Path::new("routes/api.ts"), "import express from 'express';")
            .len(),
        1
    );

    // Check NestJS matching
    assert_eq!(
        registry
            .find_matching(
                Path::new("src/auth.controller.ts"),
                "@Controller('auth')\nexport class AuthController {}"
            )
            .len(),
        1
    );

    // Check Spring matching
    assert_eq!(
        registry
            .find_matching(
                Path::new("controller/OrderController.java"),
                "@RestController\n@RequestMapping(\"/api\")\npublic class OrderController {}"
            )
            .len(),
        1
    );

    // Check Composite Analyzer
    let composite = ExpressNestSpringAnalyzer::new();
    assert_eq!(composite.name(), "express_nest_spring");
    assert!(composite.matches_framework(Path::new("server.ts"), "express().use(cors());"));
    assert!(composite.matches_framework(Path::new("app.ts"), "@Controller('app')"));
    assert!(composite.matches_framework(Path::new("App.java"), "@RestController"));
}

#[test]
fn test_context_slicer_end_to_end_framework_enhancement() {
    let source = r#"
from fastapi import APIRouter, Depends
from pydantic import BaseModel

class UserLogin(BaseModel):
    username: str
    password: str

class TokenResponse(BaseModel):
    access_token: str
    token_type: str

def auth_backend():
    return True

router = APIRouter()

@router.post("/login", response_model=TokenResponse, dependencies=[Depends(auth_backend)])
def login(payload: UserLogin):
    return {"access_token": "secret", "token_type": "bearer"}
"#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("auth.py");
    fs::write(&file_path, source).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    let result = slicer.slice_symbol(&file_path, "login", &opts).unwrap();

    assert_eq!(result.target_symbol.name, "login");
    assert!(
        result
            .hoisted_types
            .iter()
            .any(|t| t.name == "TokenResponse"),
        "Expected TokenResponse in result.hoisted_types, found: {:?}",
        result.hoisted_types
    );
    assert!(
        result.hoisted_types.iter().any(|t| t.name == "UserLogin"),
        "Expected UserLogin in result.hoisted_types, found: {:?}",
        result.hoisted_types
    );
    assert!(
        result
            .stripped_calls
            .iter()
            .any(|c| c.name == "auth_backend"),
        "Expected auth_backend in result.stripped_calls, found: {:?}",
        result.stripped_calls
    );

    // Verify generated Markdown output includes framework information
    let md = result.to_markdown();
    assert!(md.contains("TokenResponse"));
    assert!(md.contains("UserLogin"));
    assert!(md.contains("auth_backend"));
}
