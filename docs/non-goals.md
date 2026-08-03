# Non-goals

Stating non-goals early prevents the project from becoming an accumulation of unrelated language ambitions.

## Not an LLM-only language

The project will not optimize primarily for token compression, opaque generated identifiers, or structures that humans cannot reasonably inspect.

## Not a DSL

The intended result is a general-purpose programming language and semantic toolchain. The current JSON representation is an experimental transport format, not a domain-specific language or final syntax.

## Not proof of everything

The language will not pretend that hardware, firmware, operating systems, external services, compilers, or storage systems have been proven merely because a local function contract is verified. Trust boundaries must remain visible.

## Not formal proof as a universal entry requirement

Useful software must be expressible at different assurance levels. Tested, analyzed, verified, and externally verified properties may coexist, provided their distinction is explicit.

## Not an immediate replacement for Rust, Zig, C, LLVM, or assembly

Early implementations may lower through established toolchains and use foreign components. The language should make those boundaries explicit rather than claiming instant ecosystem replacement.

## Not syntax-first design

A parser, logo, file extension, and clever grammar are not the first milestone. The project must establish a semantic model that justifies a new language.

## Not a security scanner by itself

The language can make security-relevant relationships more visible, but vulnerability discovery still requires appropriate verifiers, tests, threat models, and external analysis.

## Not a guarantee that generated code is correct

Agent-generated code receives no special trust. It must satisfy the same contracts and evidence requirements as human-authored code.
