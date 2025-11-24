# Documentation Update Hook

This hook automatically updates project documentation when significant code changes are made, ensuring docs stay synchronized with implementation progress.

## Trigger Conditions

This hook runs when:
- New modules are added to src/
- Major features are completed
- Configuration schema changes
- Test coverage changes significantly
- Version bumps occur

## Hook Command

```bash
/documentation-generator:update-docs --sync implementation-status
```

## What This Hook Does

### Documentation Synchronization
1. **Analyzes Current Implementation**
   - Reviews all source files for completion status
   - Checks test coverage and validation results
   - Identifies new features and modules

2. **Updates Project Documentation**
   - README.md: Updates feature status and progress
   - CLAUDE.md: Syncs implementation progress and best practices
   - Configuration examples: Reflects current schema

3. **Implementation Status Tracking**
   - Updates completion percentages
   - Marks completed features with ✅
   - Documents tested functionality with validation data
   - Records performance metrics and insights

### Benefits

- **Always Accurate Docs**: Documentation reflects actual implementation
- **Progress Visibility**: Clear tracking of development milestones
- **Best Practices Documentation**: Captures learnings during development
- **Testing Integration**: Links docs with actual test results

## Configuration

The hook can be configured in `.claude/config.yml`:

```yaml
hooks:
  update-documentation:
    auto_run: true
    trigger_on:
      - "src/**/*.rs"
      - "Cargo.toml"
      - "tests/**/*.rs"
    exclude_patterns:
      - "target/**"
      - "*.tmp"
```

## Manual Trigger

You can manually trigger this hook by running:

```bash
/documentation-generator:update-docs --sync implementation-status
```

## Expected Output

The hook will update:
- Implementation progress percentages
- Feature completion status
- Testing results and metrics
- Best practices and insights
- Technology stack details
- Performance benchmarks

This ensures the project documentation is always an accurate reflection of the current codebase state.