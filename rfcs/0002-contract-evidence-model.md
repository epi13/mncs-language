# RFC 0002: Contract and Evidence Model

- **Status:** Implemented experimentally
- **Target:** 0.1

## Summary

Separate behavioral properties from the claims and artifacts used to support them. Require evidence to name the property and verifier it belongs to, while allowing honest gradients of assurance.

## Motivation

A binary “verified/unverified” label obscures important differences among tests, static analyses, local proofs, external reruns, and assumptions. It also encourages reports that imply whole-system certainty when only a bounded property was checked.

## Model

A contract property has a stable function-local identity, kind, and expression. An evidence claim references that property and identifies a verifier, status, and optional artifact.

Initial status vocabulary:

- claimed;
- tested;
- analyzed;
- verified;
- externally verified.

A property may have multiple evidence claims. Evidence from one verifier does not silently replace another.

## Assumptions

Assumptions are named propositions supplied outside the current boundary. Evidence should eventually enumerate the assumptions it consumes so a change can invalidate dependent claims.

## Validation obligations

- evidence cannot reference an undeclared property;
- evidence must identify a verifier;
- missing evidence is reported but not rejected by the base 0.1 profile;
- assurance profiles may later require minimum statuses for selected property classes.

## Security consequences

Binding evidence to exact properties reduces accidental or malicious reuse of unrelated outputs. It does not yet prevent artifact replacement; stable semantic identity and authenticated content hashes are required later.

## Unresolved questions

- whether assurance statuses form an order or remain independent categories;
- how evidence composition works;
- how contradictory evidence is represented;
- verifier trust and certification;
- expiration and environment-sensitive evidence;
- remote witnesses and transparency logs.
