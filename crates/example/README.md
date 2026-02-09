# Example: ReAct Agent

A file assistant that demonstrates how to build an agentic loop with Polaris.

## What it does

This example implements the ReAct pattern with the following three tools for file operations:

| Tool | Description |
|------|-------------|
| `list_files` | List files in a directory |
| `read_file` | Read the contents of a file |
| `write_file` | Write content to a file |

All paths are relative to the working directory, which acts as a sandbox. The agent cannot access files outside this directory.

## Running

```bash
# Set your API key
export ANTHROPIC_API_KEY=your-key

# Run the agent
cargo run -p example --bin react -- <working_dir> <query>

# Example
cargo run -p example --bin react -- ./sandbox "What files are here?"
```
