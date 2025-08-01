# SQLx Setup Guide

This document explains how SQLx compile-time query validation works in the automagik-forge project and how to resolve compilation issues.

## Overview

SQLx performs compile-time validation of SQL queries against your database schema. This ensures type safety and catches SQL errors at build time rather than runtime.

## For CI/GitHub Actions

The GitHub Actions workflow automatically:

1. **Creates a temporary database** in the `dev_assets` directory
2. **Runs all migrations** to set up the schema
3. **Sets DATABASE_URL** environment variable for compilation
4. **Builds the project** with full SQLx validation

No additional setup is required for CI builds.

## For Local Development

### Method 1: Run the Application First (Recommended)

The easiest way to set up the database for SQLx validation:

```bash
# Start the backend - this will create and migrate the database automatically
npm run backend:dev
```

This creates the database at `dev_assets/db.sqlite` with all migrations applied.

### Method 2: Manual Database Setup

If you need to set up the database without running the application:

```bash
# Create the dev_assets directory
mkdir -p dev_assets

# Create and migrate the database
DATABASE_URL="sqlite:dev_assets/db.sqlite" cargo sqlx database create --database-url "sqlite:dev_assets/db.sqlite"
DATABASE_URL="sqlite:dev_assets/db.sqlite" cargo sqlx migrate run --source backend/migrations
```

### Method 3: Using the Setup Script

Use the provided script to set up the database:

```bash
./generate-sqlx-data.sh
```

## Compilation with Database Validation

Once the database is set up, compile with:

```bash
# From the project root
DATABASE_URL="sqlite:dev_assets/db.sqlite" cargo check

# Or from the backend directory
DATABASE_URL="sqlite:../dev_assets/db.sqlite" cargo check
```

## Troubleshooting

### "unable to open database file" Error

This error occurs when SQLx cannot find the database file. Solutions:

1. **Check the database path**: Ensure `dev_assets/db.sqlite` exists
2. **Run the application first**: `npm run backend:dev` creates the database
3. **Use absolute paths**: Try with full path to the database file
4. **Check working directory**: Ensure you're running from the correct directory

### Compilation Errors in CI

If GitHub Actions builds fail with SQLx errors:

1. **Check the database setup step** in `.github/workflows/ci.yml`
2. **Verify migrations run successfully** in the CI logs
3. **Ensure DATABASE_URL is set correctly** for the compilation steps

### Missing Migrations

If you add new migrations:

1. **Test locally first**: Run `npm run backend:dev` to apply migrations
2. **Commit migration files**: Ensure all `.sql` files in `backend/migrations/` are committed
3. **CI will automatically apply**: New migrations are applied during CI database setup

## Database Locations

- **Development**: `dev_assets/db.sqlite` (created by running the app)
- **CI/Testing**: `dev_assets/db.sqlite` (created during GitHub Actions)
- **Production**: Configured via environment variables in deployment

## Adding New Queries

When adding new SQLx queries:

1. **Ensure database is up to date**: Run the application or migrations
2. **Test compilation locally**: Use `DATABASE_URL="sqlite:dev_assets/db.sqlite" cargo check`
3. **SQLx validates at compile time**: Queries are checked against the actual schema
4. **CI automatically validates**: New queries are tested in GitHub Actions

## Why Not Use Query Cache?

This project uses **runtime database validation** instead of SQLx's offline query cache because:

1. **Simpler setup**: No need to generate and maintain `.sqlx/` cache files
2. **Always up to date**: Queries are validated against the current schema
3. **CI-friendly**: Temporary database setup works reliably in automation
4. **Development workflow**: Running the app creates the database automatically

The trade-off is that compilation requires a database, but this is handled automatically in both local development and CI environments.