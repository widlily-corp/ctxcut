# Техническое задание (ТЗ): Проект `ctxcut` (Архитектура 2.0)

## 1. Введение и назначение
**`ctxcut`** — высокопроизводительный CLI-инструмент и MCP-сервер на языке **Rust**, предназначенный для интеллектуального AST-среза исходного кода, хирургического AST-патчинга и генерации контекста юнит-тестов с целью оптимизации работы LLM и AI-агентов (Cursor, Claude Code, Google Antigravity, ChatGPT).

Инструмент решает проблему «информационного ожирения» контекста, сокращая объем передаваемых токенов на **80–92%** при сохранении 100% семантической целостности типов, контрактов и сигнатур.

---

## 2. 6 Ключевых архитектурных столпов (6 Pillars)

### 2.1. 🚀 1. Smart Traversal & Timeout Guard (Умный обход и защита от таймаутов)
- Автоматический учет правил `.gitignore` и `.ctxcutignore`.
- Встроенный черный список артефактов сборки и кэшей: `node_modules`, `target`, `.git`, `.venv`, `.pytest_cache`, `dist`, бинарные файлы, тяжелые дампы.
- Режим моментальной поверхностной оценки токенов (`--fast` / shallow scan) без блокировки на глубоком построении AST.
- Защитные тайм-аут гарды в MCP-сервере для гарантированного ответа без зависаний.

### 2.2. 🕸️ 2. Deep Semantic / Multi-File Slicing (`--depth 1`)
- Автоматический резолвинг относительных и абсолютных локальных импортов проекта для TypeScript/JavaScript, Python, Go и Rust.
- Автоматический подъем типов и подтягивание стрипнутых сигнатур вызываемых функций из соседних файлов без необходимости дополнительных запросов со стороны AI-агента.

### 2.3. 🧩 3. Framework-Aware Semantic Intelligence (Фреймворк-ориентированная семантика)
- **Django & DRF:** Автоматический захват `Serializer`, `models.Model`, `permission_classes`, `filter_backends`, `pagination_class` при анализе ViewSet и APIView.
- **FastAPI:** Захват Pydantic-схем запроса/ответа, зависимостей `Depends(...)`, `Security(...)`, query/path параметров.
- **React & Next.js:** Автоматическое извлечение интерфейсов `Props`, кастомных хуков (`useAuth`, `useOrders` и др.) и схлопывание второстепенных веток JSX с обязательным сохранением кастомных React-компонентов.
- **Express / NestJS / Spring:** Захват DTO, декораторов контроллеров, цепочек middleware и Guards (`@UseGuards`, `@UseInterceptors`).

### 2.4. 🎯 4. Adaptive Token Budgeting (Адаптивный регулятор токенов)
- Поддержка жесткого лимита токенов: `--budget <N>` в CLI и MCP.
- Детерминированный 5-уровневый движок семантической компрессии (`BudgetCompressor`):
  1. Схлопывание второстепенных хелперов.
  2. Сжатие объемных тел switch/match ветвлений.
  3. Переход в компактный `.d.ts` / declaration стиль.
  4. Монотонное гарантированное укладывание в лимит токенов.

### 2.5. 🛠️ 5. Bidirectional AST Patcher (`ctxcut patch`)
- Хирургическая замена узлов AST в исходных файлах: `ctxcut patch <path:symbol> --with <replacement>`.
- Полное сохранение отступов окружения, комментариев и символов переноса строк (CRLF/LF).
- Строгая проверка AST-валидности через `tree-sitter` перед записью на диск, исключающая появление синтаксических ошибок.

### 2.6. 🧪 6. Isolated Test Context Generator (`ctxcut test-context`)
- Сборка изолированного контекстного бандла для генерации изолированных AAA юнит-тестов:
  1. Исходный код целевого символа.
  2. Интерфейсы входных аргументов и возвращаемых значений.
  3. Сигнатуры внешних вызовов для быстрого создания `mock` / `spy`.
  4. Образцы фикстур существующего проекта (Vitest, Jest, Pytest, Cargo test, Go test).

---

## 3. Архитектура модулей

```
crates/
├── ctxcut_core/
│   ├── traversal/      # TraversalConfig, ProjectWalker, FastStats, Blacklist, Binary
│   ├── resolver/       # Cross-file ImportResolver, CallSignatureStripper, TypeHoister
│   ├── framework/      # Django, FastAPI, React/Next, Express/Nest/Spring
│   ├── slice/          # ContextSlicer, BudgetCompressor
│   ├── patch/          # AstPatcher, IndentationAligner, SyntaxValidator
│   └── test_context/   # TestContextGenerator, FixtureFinder, MockScaffolder
├── ctxcut_cli/         # Clap CLI: slice, patch, test-context, diff, stats, metrics, route, setup-mcp
└── ctxcut_mcp/         # JSON-RPC 2.0 STDIO MCP сервер: 6 инструментов с тайм-аут гардами
```

---

## 4. Спецификация инструментов MCP (6-Pillar MCP Tools)

1. `get_symbol_slice(path, symbol, depth?, budget?)` — извлечение AST-среза с подъемом типов и межфайловым разрешением.
2. `get_diff_slice(path?, staged?, budget?)` — извлечение контекстных срезов функций из git diff.
3. `analyze_token_stats(path, fast?)` — быстрый расчет экономии токенов с учетом игнорирования.
4. `patch_symbol(path, symbol, replacement, dry_run?)` — безопасная замена символа в коде.
5. `get_test_context(path, symbol, framework?)` — генерация контекста для юнит-тестов.
6. `get_route_slice(method, route_path, budget?)` — срез веб-обработчика и DTO.

---

## 5. Стандарты надежности и тестирования
- 100% прохождение тестового набора `cargo test --all-targets` (428 тестов).
- Полное отсутствие предупреждений линтера (`cargo clippy --all-targets -- -D warnings`).
- 100% форматирование кода (`cargo fmt --check`).
