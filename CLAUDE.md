# Automagik-Forge Development Guide

## Core Architecture

- **Full-stack Rust + React monorepo** with pnpm workspace
- **Backend**: Rust/Axum API server (port 3001) with Tokio async runtime, SQLite + SQLX
- **Frontend**: React 18 + TypeScript + Vite (port 3000) with shadcn/ui components
- **Shared**: Common TypeScript types in `/shared/types.ts` generated via ts-rs
- **API**: REST endpoints at `/api/*` proxied from frontend to backend in dev
- **MCP Integration**: Dedicated MCP server for protocol communication
- **Database**: SQLite with comprehensive migrations in `backend/migrations/`

## Essential Commands

### Development Workflow
```bash
npm run dev                  # Start full-stack development (backend + frontend + MCP)
npm run check               # Run comprehensive quality checks (cargo + tsc)
npm run generate-types      # Regenerate shared TypeScript types from Rust
npm run prepare-db          # Initialize and prepare database with migrations
```

### Backend Development
```bash
npm run backend:dev:watch   # Start Rust backend in watch mode
npm run backend:check       # Run cargo check and tests
cargo build --release      # Build production binaries
```

### Frontend Development
```bash
npm run frontend:dev        # Start Vite dev server
npm run frontend:check      # Run TypeScript and lint checks
```

### MCP Development
```bash
npm run mcp:sse            # Start MCP Server-Sent Events server
```

## Code Style Standards

### Rust Conventions
- **Formatting**: Standard rustfmt with snake_case naming
- **Derives**: Always include `Debug`, `Serialize`, `Deserialize` where applicable
- **Error Handling**: Use `Result<T, E>` patterns, custom error types
- **Async**: Tokio async runtime patterns, proper async/await usage
- **Database**: SQLX with compile-time query validation

### TypeScript/React Conventions
- **TypeScript**: Strict mode enabled, interfaces over types
- **React**: Functional components with hooks, proper dependency arrays
- **Styling**: Tailwind CSS classes, shadcn/ui component library
- **Imports**: `@/` path aliases for frontend, absolute imports for backend
- **Naming**: PascalCase components, camelCase variables, kebab-case files

### Project Structure Conventions
- **Monorepo**: Workspace-based dependency management with pnpm
- **Shared Types**: Single source of truth via ts-rs generation
- **Directory Organization**: Domain-driven structure in both frontend and backend
- **Migration Management**: Sequential database migrations with rollback support

## Shared Type Management

### Type Generation System
ts-rs automatically generates TypeScript types from Rust structs/enums:

```rust
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
struct MyStruct {
    field: String,
}
```

### Type Workflow
1. **Rust Changes**: Modify structs in backend with `#[derive(TS)]` and `#[ts(export)]`
2. **Generation**: Run `npm run generate-types` to update `shared/types.ts`
3. **Validation**: Run `npm run generate-types:check` to verify type consistency
4. **Frontend Usage**: Import types from `@/lib/types` or `shared/types.ts`

**Critical Rule**: Never manually edit `shared/types.ts` - always edit `backend/src/bin/generate_types.rs`

## Full-Stack Development Workflow

### Development Order Protocol
1. **Backend First**: Always start with backend changes for full-stack features
2. **Type Generation**: Regenerate shared types after backend struct changes
3. **Frontend Integration**: Implement frontend changes using updated types
4. **Quality Validation**: Run `npm run check` before committing

### Database Evolution
1. **Migration Creation**: Add new migrations to `backend/migrations/`
2. **Database Preparation**: Run `npm run prepare-db` to apply migrations
3. **Model Updates**: Update corresponding models in `backend/src/models/`
4. **Type Generation**: Regenerate types if database changes affect API responses

## Quality Assurance Framework

### Comprehensive Testing Strategy
```bash
npm run check               # Full quality gate (cargo + TypeScript checks)
cargo test                  # Run Rust unit and integration tests
npm run frontend:check      # TypeScript compilation and lint validation
```

### Pre-Commit Quality Gates
- **Rust Quality**: Cargo check, clippy lints, formatting validation
- **TypeScript Quality**: Strict compilation, ESLint validation
- **Type Consistency**: Shared type generation validation
- **Database Integrity**: Migration validation and model consistency

### Evidence-Based Development
- **Comprehensive Testing**: Unit, integration, and end-to-end test coverage
- **Type Safety**: Compile-time validation across Rust and TypeScript boundaries
- **Database Validation**: SQLX compile-time query validation
- **API Consistency**: Shared type definitions ensure frontend/backend alignment

## Backend Architecture Details

### Data Model Organization
- **Location**: All SQLX queries in `backend/src/models/*`
- **Pattern**: Use model getters/setters instead of raw SQL queries
- **Migration Management**: Sequential migrations with proper rollback support
- **Type Safety**: SQLX compile-time query validation enabled

### Service Layer Architecture
```
backend/src/
├── models/          # Data access layer with SQLX queries
├── services/        # Business logic layer
├── routes/          # HTTP API endpoints
├── executors/       # Task execution implementations
├── security/        # Security and audit implementations
└── utils/           # Shared utility functions
```

### Security Implementation
- **Audit Logging**: Comprehensive audit trail in `security/audit_logger.rs`
- **Session Security**: Secure session management with token encryption
- **Security Headers**: Proper HTTP security headers implementation
- **Authentication**: OAuth2 integration with GitHub authentication

## Frontend Architecture Details

### Component Organization
```
frontend/src/
├── components/      # Reusable UI components
├── pages/          # Application pages and routing
├── hooks/          # Custom React hooks
├── services/       # API client implementations
├── lib/            # Utility functions and configurations
└── context/        # React context providers
```

### State Management Patterns
- **Local State**: React hooks (useState, useReducer) for component state
- **Context**: React context for shared state across component trees
- **API State**: Custom hooks for API data fetching and caching
- **Real-time Updates**: WebSocket/SSE integration for live collaboration

## Advanced Development Patterns

### Workspace Management
- **Monorepo Structure**: pnpm workspace with shared dependencies
- **Development Environment**: Automated port management and environment setup
- **Build Coordination**: Coordinated build processes for frontend and backend
- **Dependency Management**: Shared dependencies managed at workspace level

### MCP Integration Patterns
- **Protocol Implementation**: Dedicated MCP server for Claude integration
- **Task Management**: Sophisticated task execution and monitoring systems
- **Real-time Communication**: Server-Sent Events for live updates
- **Error Handling**: Comprehensive error handling and recovery mechanisms

### Performance Optimization
- **Backend Performance**: Async/await patterns, efficient database queries
- **Frontend Performance**: Code splitting, lazy loading, component optimization
- **Build Optimization**: Optimized build processes and asset management
- **Development Performance**: Watch mode optimization and incremental builds

## Troubleshooting Guide

### Common Development Issues

#### Type Generation Problems
**Symptoms**: TypeScript errors about missing types, build failures
**Solutions**:
1. Run `npm run generate-types` after Rust struct changes
2. Check `backend/src/bin/generate_types.rs` for proper exports
3. Verify `shared/types.ts` has been updated correctly
4. Ensure Rust structs have proper `#[derive(TS)]` and `#[ts(export)]` annotations

#### Database Migration Issues
**Symptoms**: Database connection errors, schema mismatches
**Solutions**:
1. Run `npm run prepare-db` to apply pending migrations
2. Check migration files in `backend/migrations/` for syntax errors
3. Verify database file permissions and location
4. Use SQLX offline mode for compilation without database connection

#### MCP Connection Problems
**Symptoms**: "Failed to reconnect to automagik-forge", protocol errors
**Solutions**:
1. Check transport mode: `--mcp` for STDIO, no flags for SSE
2. Verify port availability: Default SSE port is 8889
3. Test binary extraction: Check `npx-cli/dist/{platform}/` directory
4. Validate MCP server startup: Check server logs for initialization errors

#### Build and Compilation Issues
**Symptoms**: Compilation errors, missing dependencies, build failures
**Solutions**:
1. Run `npm run check` to identify specific issues
2. Verify all dependencies are installed: `pnpm install`
3. Check Rust toolchain: `rustc --version` and `cargo --version`
4. Clear build artifacts: `cargo clean` and `rm -rf frontend/dist`

### Development Environment Setup
1. **Prerequisites**: Node.js >=18, pnpm >=8, Rust toolchain
2. **Installation**: `pnpm install` for all dependencies
3. **Database Setup**: `npm run prepare-db` for database initialization
4. **Development Start**: `npm run dev` for full-stack development

### Quality Assurance Checklist
- [ ] All tests pass: `npm run check`
- [ ] Types are synchronized: `npm run generate-types:check`
- [ ] Database migrations applied: `npm run prepare-db`
- [ ] No compilation warnings in Rust or TypeScript
- [ ] MCP integration functional for target use case
- [ ] Frontend builds without errors: `npm run frontend:check`
- [ ] Backend builds without errors: `npm run backend:check`

## Production Deployment

### Build Process
```bash
cargo build --release                    # Build optimized Rust binary
npm run frontend:build                   # Build optimized frontend assets
./build-npm-package.sh                   # Create distributable NPX package
```

### Deployment Verification
- **Binary Testing**: Verify binary functionality across target platforms
- **Package Testing**: Test NPX package installation and execution
- **MCP Integration**: Validate MCP protocol compatibility
- **Database Migrations**: Ensure production migration compatibility

This guide provides comprehensive development guidance for the automagik-forge ecosystem, emphasizing quality, type safety, and efficient full-stack development workflows.