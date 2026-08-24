//! Comprehensive Milestone 2 verification suite covering C/C++, C#, Java, Kotlin, and Vue/Svelte/Astro SFCs.

use ctxcut_core::lang::sfc::{SfcBlockKind, SfcDocument};
use ctxcut_core::lang::LanguageRegistry;
use ctxcut_core::model::{SliceOptions, SupportedLanguage};
use ctxcut_core::parser::ParserManager;
use ctxcut_core::resolver::implementors::ImplementorHoister;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_c_cpp_symbol_and_type_slicing() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let header_path = root.join("types.hpp");
    let header_src = r#"
#pragma once
#include <string>

struct UserDto {
    uint64_t id;
    std::string username;
    bool active;
};

class IUserService {
public:
    virtual ~IUserService() = default;
    virtual UserDto GetUser(uint64_t id) = 0;
};
"#;
    fs::write(&header_path, header_src).unwrap();

    let cpp_path = root.join("user_service.cpp");
    let cpp_src = r#"
#include "types.hpp"
#include <iostream>

class UserService : public IUserService {
public:
    UserDto GetUser(uint64_t id) override {
        UserDto u;
        u.id = id;
        u.username = "alice";
        u.active = true;
        return u;
    }
};
"#;
    fs::write(&cpp_path, cpp_src).unwrap();

    let adapter = LanguageRegistry::for_language(SupportedLanguage::Cpp).unwrap();
    let ts_lang = adapter.tree_sitter_language(&cpp_path);
    let tree = ParserManager::parse_source(cpp_src, &ts_lang, &cpp_path).unwrap();

    // 1. Locate method
    let (sym, node) = adapter
        .locate_symbol(tree.root_node(), cpp_src, "UserService::GetUser", &cpp_path)
        .unwrap();
    assert_eq!(sym.name, "UserService::GetUser");
    assert_eq!(sym.language, "cpp");

    // 2. Transitive type hoisting
    let hoisted = adapter
        .hoist_types(
            node,
            tree.root_node(),
            cpp_src,
            &cpp_path,
            &SliceOptions::default(),
        )
        .unwrap();
    assert!(hoisted.iter().any(|t| t.name == "UserDto"));

    // 3. Implementor discovery
    let implementors = ImplementorHoister::find_implementors(
        root,
        &header_path,
        "IUserService",
        SupportedLanguage::Cpp,
    )
    .unwrap();
    assert!(implementors
        .iter()
        .any(|imp| imp.implementor_name.contains("UserService")));
}

#[test]
fn test_csharp_symbol_aspnet_and_type_slicing() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let models_path = root.join("Models.cs");
    let models_src = r#"
namespace MyApp.Models;

public record OrderDto(long Id, string Customer, decimal Total);

public interface IOrderService {
    OrderDto CreateOrder(OrderDto order);
}
"#;
    fs::write(&models_path, models_src).unwrap();

    let controller_path = root.join("OrdersController.cs");
    let controller_src = r#"
using Microsoft.AspNetCore.Mvc;
using MyApp.Models;

namespace MyApp.Controllers;

[ApiController]
[Route("api/[controller]")]
public class OrdersController : ControllerBase {
    private readonly IOrderService _service;

    public OrdersController(IOrderService service) {
        _service = service;
    }

    [HttpPost]
    public ActionResult<OrderDto> CreateOrder([FromBody] OrderDto request) {
        var created = _service.CreateOrder(request);
        return Ok(created);
    }
}
"#;
    fs::write(&controller_path, controller_src).unwrap();

    let adapter = LanguageRegistry::for_language(SupportedLanguage::CSharp).unwrap();
    let ts_lang = adapter.tree_sitter_language(&controller_path);
    let tree = ParserManager::parse_source(controller_src, &ts_lang, &controller_path).unwrap();

    // 1. Locate action
    let (sym, node) = adapter
        .locate_symbol(
            tree.root_node(),
            controller_src,
            "OrdersController.CreateOrder",
            &controller_path,
        )
        .unwrap();
    assert_eq!(sym.name, "OrdersController.CreateOrder");
    assert_eq!(sym.language, "csharp");

    // 2. Hoist DTO type
    let hoisted = adapter
        .hoist_types(
            node,
            tree.root_node(),
            controller_src,
            &controller_path,
            &SliceOptions::default(),
        )
        .unwrap();
    assert!(hoisted.iter().any(|t| t.name == "OrderDto"));

    // 3. Strip calls
    let stubs = adapter
        .strip_calls(node, tree.root_node(), controller_src, &controller_path)
        .unwrap();
    assert!(stubs.iter().any(|s| s.name == "CreateOrder"));
}

#[test]
fn test_java_symbol_and_implementor_slicing() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let interface_path = root.join("PaymentGateway.java");
    let interface_src = r#"
package com.example.payment;

public interface PaymentGateway {
    PaymentResult process(PaymentRequest req);
}
"#;
    fs::write(&interface_path, interface_src).unwrap();

    let impl_path = root.join("StripeGateway.java");
    let impl_src = r#"
package com.example.payment;

public class StripeGateway implements PaymentGateway {
    @Override
    public PaymentResult process(PaymentRequest req) {
        return new PaymentResult(true, "tx_123");
    }
}
"#;
    fs::write(&impl_path, impl_src).unwrap();

    let adapter = LanguageRegistry::for_language(SupportedLanguage::Java).unwrap();
    let ts_lang = adapter.tree_sitter_language(&impl_path);
    let tree = ParserManager::parse_source(impl_src, &ts_lang, &impl_path).unwrap();

    // 1. Locate method
    let (sym, _node) = adapter
        .locate_symbol(
            tree.root_node(),
            impl_src,
            "StripeGateway.process",
            &impl_path,
        )
        .unwrap();
    assert_eq!(sym.name, "StripeGateway.process");
    assert_eq!(sym.language, "java");

    // 2. Find implementor
    let implementors = ImplementorHoister::find_implementors(
        root,
        &interface_path,
        "PaymentGateway",
        SupportedLanguage::Java,
    )
    .unwrap();
    assert!(implementors
        .iter()
        .any(|imp| imp.implementor_name == "StripeGateway"));
}

#[test]
fn test_kotlin_symbol_extension_and_type_slicing() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let models_path = root.join("UserModels.kt");
    let models_src = r#"
package com.example.user

data class UserSession(val token: String, val userId: Long)

interface AuthRepository {
    fun authenticate(token: String): UserSession?
}
"#;
    fs::write(&models_path, models_src).unwrap();

    let service_path = root.join("AuthService.kt");
    let service_src = r#"
package com.example.user

class DefaultAuthService(private val repo: AuthRepository) : AuthRepository {
    override fun authenticate(token: String): UserSession? {
        return repo.authenticate(token)
    }

    fun UserSession.isValid(): Boolean {
        return token.isNotEmpty()
    }
}
"#;
    fs::write(&service_path, service_src).unwrap();

    let adapter = LanguageRegistry::for_language(SupportedLanguage::Kotlin).unwrap();
    let ts_lang = adapter.tree_sitter_language(&service_path);
    let tree = ParserManager::parse_source(service_src, &ts_lang, &service_path).unwrap();

    // 1. Locate method
    let (sym, node) = adapter
        .locate_symbol(
            tree.root_node(),
            service_src,
            "DefaultAuthService.authenticate",
            &service_path,
        )
        .unwrap();
    assert_eq!(sym.name, "DefaultAuthService.authenticate");
    assert_eq!(sym.language, "kotlin");

    // 2. Hoist UserSession
    let hoisted = adapter
        .hoist_types(
            node,
            tree.root_node(),
            service_src,
            &service_path,
            &SliceOptions::default(),
        )
        .unwrap();
    assert!(hoisted.iter().any(|t| t.name == "UserSession"));

    // 3. Find implementors
    let implementors = ImplementorHoister::find_implementors(
        root,
        &models_path,
        "AuthRepository",
        SupportedLanguage::Kotlin,
    )
    .unwrap();
    assert!(implementors
        .iter()
        .any(|imp| imp.implementor_name == "DefaultAuthService"));
}

#[test]
fn test_vue_sfc_segmentation_and_props_slicing() {
    let vue_src = r#"<template>
  <div class="user-card">
    <h2>{{ user.name }}</h2>
    <p>{{ user.email }}</p>
    <button @click="emit('update', user.id)">Save</button>
  </div>
</template>

<script setup lang="ts">
import { defineProps, defineEmits } from 'vue';

export interface UserProps {
  id: number;
  name: string;
  email: string;
}

const props = defineProps<{
  user: UserProps;
  active?: boolean;
}>();

const emit = defineEmits<{(e: 'update', id: number): void}>();

function formatName(name: string): string {
  return name.trim().toUpperCase();
}
</script>

<style scoped>
.user-card {
  padding: 1rem;
  border-radius: 8px;
}
</style>
"#;

    let doc = SfcDocument::parse_vue(vue_src);
    assert!(doc
        .blocks
        .iter()
        .any(|b| b.kind == SfcBlockKind::ScriptSetup));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Template));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Style));

    let summaries = doc.collapse_summaries();
    assert!(summaries.iter().any(|s| s.contains("<template>")));
    assert!(summaries.iter().any(|s| s.contains("style")));

    let adapter = LanguageRegistry::for_language(SupportedLanguage::Vue).unwrap();
    let temp_path = Path::new("UserCard.vue");
    let ts_lang = adapter.tree_sitter_language(temp_path);
    let tree = ParserManager::parse_source(vue_src, &ts_lang, temp_path).unwrap();

    let (sym, _) = adapter
        .locate_symbol(tree.root_node(), vue_src, "formatName", temp_path)
        .unwrap();
    assert_eq!(sym.name, "formatName");
    assert_eq!(sym.language, "vue");

    let (props_sym, _) = adapter
        .locate_symbol(tree.root_node(), vue_src, "Props", temp_path)
        .unwrap();
    assert!(
        props_sym.name.contains("Props")
            || props_sym.name == "UserProps"
            || props_sym.body.contains("defineProps")
    );
}

#[test]
fn test_svelte_sfc_segmentation_and_props_slicing() {
    let svelte_src = r#"<script lang="ts">
  export let count: number = 0;
  export let label: string = "Clicks";

  function increment() {
    count += 1;
  }
</script>

<button on:click={increment}>
  {label}: {count}
</button>

<style>
  button {
    background: #ff3e00;
    color: white;
  }
</style>
"#;

    let doc = SfcDocument::parse_svelte(svelte_src);
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Script));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Markup));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Style));

    let summaries = doc.collapse_summaries();
    assert!(summaries.iter().any(|s| s.contains("<style>")));

    let adapter = LanguageRegistry::for_language(SupportedLanguage::Svelte).unwrap();
    let temp_path = Path::new("Counter.svelte");
    let ts_lang = adapter.tree_sitter_language(temp_path);
    let tree = ParserManager::parse_source(svelte_src, &ts_lang, temp_path).unwrap();

    let (sym, _) = adapter
        .locate_symbol(tree.root_node(), svelte_src, "increment", temp_path)
        .unwrap();
    assert_eq!(sym.name, "increment");
    assert_eq!(sym.language, "svelte");

    let (prop_sym, _) = adapter
        .locate_symbol(tree.root_node(), svelte_src, "count", temp_path)
        .unwrap();
    assert_eq!(prop_sym.name, "count");
}

#[test]
fn test_astro_sfc_segmentation_and_props_slicing() {
    let astro_src = r#"---
interface Props {
  title: string;
  description?: string;
}

const { title, description = "Default description" } = Astro.props;

function getPageMetadata(): { title: string; desc: string } {
  return { title, desc: description };
}
---

<div class="header">
  <h1>{title}</h1>
  <p>{description}</p>
</div>

<style>
  h1 {
    font-size: 2rem;
  }
</style>
"#;

    let doc = SfcDocument::parse_astro(astro_src);
    assert!(!doc.combined_script.is_empty());
    assert!(doc
        .blocks
        .iter()
        .any(|b| b.kind == SfcBlockKind::Frontmatter));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Markup));

    let summaries = doc.collapse_summaries();
    assert!(summaries.iter().any(|s| s.contains("Markup")));

    let adapter = LanguageRegistry::for_language(SupportedLanguage::Astro).unwrap();
    let temp_path = Path::new("Header.astro");
    let ts_lang = adapter.tree_sitter_language(temp_path);
    let tree = ParserManager::parse_source(astro_src, &ts_lang, temp_path).unwrap();

    let (sym, _) = adapter
        .locate_symbol(tree.root_node(), astro_src, "getPageMetadata", temp_path)
        .unwrap();
    assert_eq!(sym.name, "getPageMetadata");
    assert_eq!(sym.language, "astro");

    let (props_sym, _) = adapter
        .locate_symbol(tree.root_node(), astro_src, "Props", temp_path)
        .unwrap();
    assert_eq!(props_sym.name, "Props");
}
