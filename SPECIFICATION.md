# Техническая спецификация (Архитектура 2.0): Проект `ctxcut`

## 1. Введение и назначение

**`ctxcut v2.0`** — высокопроизводительный движок на языке **Rust**, предоставляющий средства контекстного AST-среза исходного кода, обратного анализа вызовов (`callers`), сквозной трассировки потока выполнения (`trace`), полиглотного подъема конкретных имплементаций, сшивания ORM-схем и DDL-миграций, верификационного AST-патчинга с авто-откатом, семантического diff, рефакторинга символов, персистентного SQLite-индексирования, структурного AST-поиска (`query`) и терминального TUI-дашборда для больших языковых моделей (LLM) и AI-агентов (Google Antigravity, Cursor, Claude Code, Cline, Roo Code, ChatGPT).

### 1.1. Главная цель
Полное устранение проблемы «информационного ожирения» (Context Obesity) контекста при работе LLM с кодом:
- Снижение объема передаваемых токенов на **80–92%** при срезах функций и на **90–95%** при обзоре репозитория.
- Сохранение 100% синтаксической валидности, контрактов типов и сигнатур зависимостей.
- Исключение деградации внимания (*Lost-in-the-Middle*) за счёт удаления избыточных тел функций и второстепенного бойлерплейта.
- Сквозное понимание цепочек вызовов (от HTTP-роута до базы данных) без прочитывания сотен файлов целиком.

---

## 2. Архитектурная карта и функциональный инвентарь

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SOURCE CODE / REPOSITORY                          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                ┌──────────────────────▼──────────────────────┐
                │ 1. Smart Traversal & Persistent Index       │
                │    (.gitignore, SQLite WAL .ctxcut/index.db)│
                └──────────────────────┬──────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
┌───────▼────────────────┐   ┌─────────▼──────────────┐   ┌───────────▼────────────┐
│ 2. Deep Graph & Flow   │   │ 3. Polyglot & SFC Lang │   │ 4. ORM & Schema Stitch │
│  • Upstream `callers`  │   │  • TS, JS, Python, Go  │   │  • Prisma models & DDL │
│  • `trace` execution   │   │  • Rust, C/C++, C#/.NET│   │  • Drizzle & TypeORM   │
│  • Implementor Hoist   │   │  • Java, Kotlin        │   │  • SQL Migrations DDL  │
│  • Multi-symbol batch  │   │  • Vue, Svelte, Astro  │   │  • Proto & GraphQL SDL │
└───────┬────────────────┘   └─────────┬──────────────┘   └───────────┬────────────┘
        │                              │                              │
        └──────────────────────────────┼──────────────────────────────┘
                                       │
                ┌──────────────────────▼──────────────────────┐
                │ 5. Adaptive Token Budgeting                 │
                │    (5-level progressive semantic degradation│
                └──────────────────────┬──────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
┌───────▼────────────────┐   ┌─────────▼──────────────┐   ┌───────────▼────────────┐
│ 6. Verification Guard  │   │ 7. Semantic AST Diff   │   │ 8. Structural Query    │
│  • AST Syntax check    │   │  • Signature/type delta│   │  • Tree-sitter S-expr  │
│  • Compiler dry-run    │   │  • Token ROI metrics   │   │  • AST Presets         │
│  • RAII auto-rollback  │   │  • Refactor & Rename   │   │  • Interactive TUI     │
└───────┬────────────────┘   └─────────┬──────────────┘   └───────────┬────────────┘
        │                              │                              │
        └──────────────────────────────┴──────────────────────────────┘
                                       │
         ┌─────────────────────────────▼─────────────────────────────┐
         │              DELIVERY INTERFACES & PLATFORMS              │
         │  • Unified CLI (`slice`, `callers`, `trace`, `query`, ...)│
         │  • Model Context Protocol (STDIO JSON-RPC 2.0 Server)     │
         │  • Interactive Ratatui Terminal UI Dashboard (`tui`)      │
         │  • IDE Auto-Config (Antigravity, Cursor, Claude, VSCode)  │
         └───────────────────────────────────────────────────────────┘
```

---

## 3. Матрица функциональных модулей (Milestones 1–5)

### 3.1. Milestone 1: Deep Graph & Control Flow
- **`ctxcut callers` / `get_impact_slice` (F1):** Обратный срез точек вызова по всей кодовой базе с выделением вызывающих строк и окружающего контекста.
- **`ctxcut trace` / `get_trace_slice` (F2):** Сквозная трассировка цепочки выполнения от роутов/точек входа вглубь сервисов и БД в рамках адаптивного бюджета (1000–2000 токенов).
- **Implementor Hoisting (F3):** Автоматический поиск и подъем конкретных структур и классов, реализующих интерфейсы/трейты:
  - Rust: `impl Trait for Struct`
  - Go: структурная утиная типизация (matching method receiver sets)
  - TypeScript: `class C implements Interface`
  - Python: `Protocol` и номинальное наследование

### 3.2. Milestone 2: Multi-Language & SFC Grammar Expansion
- **C / C++ (F4):** Разбор классов, структур, шаблонов `template<...>`, типов возврата и декомпозиция `#include` и макросов.
- **C# / .NET (F5):** ASP.NET Core `[ApiController]` и маршруты, `record`, `struct`, `interface`, DTO.
- **Java & Kotlin (F6):** Spring Boot аннотации (`@RestController`, `@GetMapping`), JPA entities, wildcard/reified generics, Kotlin extension-функции.
- **Single File Components (SFC) (F7):**
  - Vue: выделение `<script setup>` и пропсов, компактное сворачивание `<template>` и `<style>`.
  - Svelte: выделение `<script>` и рун Svelte 5 (`$props`).
  - Astro: выделение фронтматтера `---` и директив клиента.

### 3.3. Milestone 3: ORM, Database & API Schema Stitching (F8)
- **Prisma:** Парсинг `schema.prisma`, извлечение `model` и связей при обнаружении вызовов `prisma.<model>.<method>`.
- **Drizzle ORM:** Извлечение `pgTable`, `mysqlTable`, `sqliteTable` из `schema.ts`.
- **TypeORM:** Подъем классов сущностей `@Entity()`.
- **SQL Migrations:** Парсинг DDL `CREATE TABLE` из `migrations/*.sql` при обнаружении `sqlx::query!`, `db.query(...)`.
- **Protobuf & GraphQL:** Сшивание сообщений `.proto` (gRPC) и GraphQL SDL (`.graphql`) для резолверов.

### 3.4. Milestone 4: Verification Guard, Semantic Diff & Refactoring
- **Verification Guard (`ctxcut verify-patch`, F9):** Dry-run применение патча с вызовом компилятора (`cargo check`, `tsc --noEmit`, `mypy`) и автоматическим RAII-откатом при ошибках.
- **Semantic AST Diff (`ctxcut semantic-diff`, F10):** Структурный дифф AST, выделяющий изменения в сигнатурах, типах и полях, с точным расчетом экономии BPE токенов.
- **AST Refactor Rename (`ctxcut refactor rename`, F11):** Мультифайловое переименование символов на уровне AST без риска задеть ложноположительные текстовые совпадения.

### 3.5. Milestone 5: Persistent Indexing, AST Query Engine & TUI
- **SQLite WAL Cache (`.ctxcut/index.db`, F12):** Инкрементальное кэширование символов и AST-хэшей для отклика `< 5 мс` на монорепозиториях.
- **AST Query Engine (`ctxcut query`, F13):** Структурный поиск по кодовой базе через S-выражения Tree-sitter и готовые пресеты (`functions`, `types`, `routes`, `calls`, `classes`).
- **Interactive TUI Dashboard (`ctxcut tui`, F14):** Терминальный дашборд на `ratatui` для визуализации AST-срезов и телеметрии.
- **Release Automation (`ctxcut upgrade`, F15):** Самообновление и проверка версий.

---

## 4. Матрица CLI-подкоманд (20 Subcommands)

| Подкоманда | Назначение | Ключевые параметры |
| :--- | :--- | :--- |
| `slice` | Извлечение AST-среза одного или нескольких символов | `<target>`, `--budget`, `--depth`, `--no-types`, `--no-calls`, `--clip`, `-o`, `--format` |
| `callers` | Обратный срез точек вызова по всей кодовой базе | `<target>`, `--budget`, `--limit`, `--clip`, `-o`, `--format` |
| `trace` | Сквозная трассировка цепочки выполнения до БД | `<entry>`, `--budget`, `--depth`, `--clip`, `-o`, `--format` |
| `query` | Поиск по AST-паттернам Tree-sitter и пресетам | `[<pattern>]`, `--preset`, `--lang`, `--limit`, `--root` |
| `verify-patch` | Проверка патча с запуском компилятора и авто-откатом | `<target>`, `-c/--code`, `-f/--file`, `--typecheck-cmd`, `--dry-run` |
| `semantic-diff` | Структурный AST-дифф с метриками ROI токенов | `[<path>]`, `--staged`, `--budget`, `--format` |
| `refactor` | AST-точное переименование символов в кодовой базе | `rename <target> --to <NEW_NAME>`, `--dry-run` |
| `index` | Управление SQLite-кэшем (`.ctxcut/index.db`) | `--clear`, `--stats` |
| `tui` / `dashboard` | Интерактивный терминальный дашборд на Ratatui | `--refresh <MS>` |
| `diff` | Срез функций, измененных в git diff | `--staged`, `--budget`, `--clip`, `-o`, `--format` |
| `route` | Разрешение HTTP-маршрута до обработчика и DTO | `<method>`, `<path>`, `--budget`, `--clip`, `-o`, `--format` |
| `patch` | Хирургическая замена символа в исходном коде | `<target>`, `-c/--code`, `-f/--file`, `--dry-run` |
| `test-context` | Бандл контекста для юнит-тестирования | `<target>`, `--framework`, `--budget`, `--clip`, `-o`, `--format` |
| `stats` | Анализ потенциала оптимизации токенов | `[<path>]`, `-f/--fast`, `--history`, `--format` |
| `metrics` | Интерактивный дашборд экономии токенов и ROI | `--format <text\|json>` |
| `overview` | Индексация архитектуры и символов проекта | `[<path>]`, `--depth`, `--budget`, `--format` |
| `setup-mcp` | Автоконфигурация MCP-сервера в IDE | `--ide`, `--workspace`, `--workspace-dir`, `--custom-path`, `--dry-run` |
| `init` | Инициализация и настройка MCP (алиас `setup-mcp`) | Те же флаги, что и `setup-mcp` |
| `upgrade` | Проверка и самообновление бинарника | `--check` |
| `mcp` | Запуск сервера протокола MCP через STDIO | `--log-file <PATH>` |

---

## 5. Каталог инструментов MCP (10 Tools)

| Tool Name | Входные параметры | Описание |
| :--- | :--- | :--- |
| `get_symbol_slice` | `path`, `symbol`, `depth`, `budget`, `no_types`, `no_calls` | Извлечение AST-среза с типами, имплементаторами, ORM-схемами и сигнатурами вызовов. |
| `get_impact_slice` | `symbol`, `path`, `root_dir`, `budget`, `limit` | Обратный анализ вызовов по проекту для оценки эффекта изменений целевого символа. |
| `get_trace_slice` | `entry`, `root_dir`, `depth`, `budget` | Сквозная трассировка цепочки выполнения от точек входа до баз данных и сервисов. |
| `get_diff_slice` | `path`, `staged`, `budget` | Автоматический срез функций, затронутых в Git working tree или staged diff. |
| `get_workspace_overview` | `path`, `depth`, `budget` | Архитектурный очерк проекта без чтения тел функций (90–95% экономии токенов). |
| `get_route_slice` | `method`, `path`, `root_dir`, `budget` | Поиск и срез обработчика API-маршрута, DTO-схем и middleware. |
| `get_test_context` | `path`, `symbol`, `framework`, `budget` | Формирование изолированного тестового контекста с моками и фикстурами. |
| `patch_symbol` | `path`, `symbol`, `code`, `dry_run` | Хирургическая замена AST-узла с выравниванием отступов и проверкой синтаксиса. |
| `analyze_token_stats` | `path`, `fast` | Расчет экономии токенов для файла или репозитория. |
| `get_metrics` | `format`, `clear` | Инспекция накопленной телеметрии экономии токенов и долларового ROI. |

---

## 6. Стандарты надежности и качества (705+ тестов)

- **Tier 1 (298 тестов):** Полнота покрытия всех 15 фич (Callers, Trace, Implementors, C/C++, C#, JVM, SFC, ORM, Verify, Diff, Rename, SQLite, Query, TUI, Upgrade).
- **Tier 2 (250 тестов):** Граничные условия (циклические импорты, синтаксические сбои, Unicode-пути, сверхбольшие файлы).
- **Tier 3 (74 теста):** Кросс-функциональные комбинации парных фич.
- **Tier 4 (63 теста):** Реальные микросервисные нагрузки (E-Commerce, Auth, Billing, Inventory).
- **Tier 5 (20 тестов):** Телеметрия, отказоустойчивость STDIO, многопоточный стресс.
- **Unit & Benches (35+):** Специфические тесты языковых адаптеров, сшивателей схем и Criterion-бенчмарки.

Инженерные инварианты:
- `#![deny(unsafe_code)]`
- `cargo clippy --workspace --all-targets -- -D warnings` (0 ворнингов)
- 100% прохождение всех тестов в CI/CD.
