# Family provider metadata

`family-provider-source-v1.json` is the compiler-owned export manifest for
the language's three shared compiler/runtime contracts:

- compiler test inventory;
- bounded semantic impact;
- execution observations.

`scripts/generate_family_provider_metadata.py` binds that manifest to a
content-addressed interface identity and emits
`family-provider-metadata-v1.json`. It also checks that the reviewable
`family-semantic-contracts-v1.json` declaration has exactly the same
`provides` facts. This keeps provider facts generated while leaving consumer
intent and family edges human-reviewable.

Run:

```sh
python3 scripts/generate_family_provider_metadata.py --check
PYTHONPATH=scripts python3 scripts/test_family_provider_metadata.py
```

The generated authority kind is
`language-owned-export-manifest`, because these are compiler command/result
contracts rather than one MNCS callable interface. Callable MNCS surfaces use
the Commons `language-owned-abi` authority instead.
