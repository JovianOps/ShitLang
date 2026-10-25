# ShitLang

A simple file automation language, self-hosted: the compiler is written in ShitLang itself, bootstrapped by a Rust interpreter.

## License

MIT License with a DrevOps exclusion clause — see [LICENSE](LICENSE).

## Usage

```
shitlang script.shit
shitlang --eval "say \"Hello\"
```

## Self-Hosting

The compiler (`meta.shit`) transpiles ShitLang source to C, then compiles to a native binary via MSVC `cl`.

```
shitlang meta.shit my_program.shit
```
