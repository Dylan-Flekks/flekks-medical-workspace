# Patient identity, contact, and coverage model

This design note describes the synthetic-data identity and coverage boundary in Flekks Medical
Workspace. The feature is an auditable local research workflow, not an eligibility service,
clearinghouse, billing system, or production identity-verification product.

## Why structured identity is separate from display identity

The patient chart keeps both a provider-friendly display name and structured legal/billing name
components. A preferred or display name is useful in care; it is not a substitute for the name
printed on a coverage card. CMS instructs claim submitters to enter a Medicare beneficiary's name
exactly as it appears on the Medicare card, and warns that mismatched identifying information can
make a claim unprocessable.

References:

- [CMS Medicare Billing: Form CMS-1500 and the 837 Professional](https://www.cms.gov/Outreach-and-Education/MLN/WBT/MLN4462429-MLN-WBT-1500/1500/lesson04/15/index.html)
- [CMS Medicare Claims Processing Manual, Chapter 26](https://www.cms.gov/manuals/downloads/clm104c26.pdf)
- [OpenEMR claim data fields](https://www.open-emr.org/wiki/index.php/List_of_OpenEMR_Data_Fields_Required_For_Insurance_Claims)
- [HL7 FHIR R4 Patient](https://hl7.org/fhir/R4/patient-definitions.html)
- [HL7 FHIR R4 Coverage](https://hl7.org/fhir/R4/coverage-definitions.html)

## Patient record

The patient-rooted record contains:

- display name, preferred name, previous or alias name;
- legal first, middle, and last name plus suffix;
- date of birth, administrative sex, patient ID or MRN;
- preferred language and whether an interpreter is needed;
- primary and secondary phone plus use labels;
- primary and secondary email;
- preferred contact method;
- home/mailing address lines, city, state or province, postal code, country, and use;
- emergency-contact name, relationship, phone, and email;
- local contact notes.

The initial implementation intentionally does not claim US Core conformance and does not yet add
race, ethnicity, tribal affiliation, marital status, gender identity, or pronouns. Those require a
separate terminology, provenance, consent, and workflow design pass.

## Coverage records

A patient can have up to three ordered coverage records: primary, secondary, and tertiary. Each
record stores:

- payer and plan name;
- member or Medicare identifier and group number;
- coverage type, status, effective date, and termination date;
- relationship of the patient to the subscriber;
- subscriber legal name, date of birth, and administrative sex;
- subscriber address, or an explicit same-as-patient flag;
- local coverage notes;
- opaque version and creation/update timestamps.

The legacy coverage fields on the patient response remain a compatibility projection of primary
coverage. New code should use the ordered coverage API.

## Human-entered card comparison

The workspace does not run OCR or ask a model to infer identity from a card. A clinician or
authorized synthetic-data tester chooses the coverage and source document, then transcribes the
printed name and member identifier. The comparison:

1. requires a present, hashed, patient-owned local insurance-card reference;
2. pins the current patient, coverage, and document-record versions plus the card content hash;
3. compares a self/Medicare beneficiary card with the patient's structured legal identity;
4. compares a dependent card with the structured subscriber identity;
5. normalizes only Unicode-safe case and repeated whitespace for the match decision;
6. preserves punctuation, apostrophes, hyphens, middle names, and suffixes as meaningful fields;
7. records the compared fields, mismatches, actor, source document, timestamp, and content hash in
   an append-only verification event.

Medicare Beneficiary Identifiers receive a format check based on the CMS 11-character MBI format.
A format result is not an eligibility result and does not prove active coverage.

References:

- [CMS Understanding the Medicare Beneficiary Identifier](https://www.cms.gov/medicare/new-medicare-card/understanding-the-mbi.pdf)
- [CMS Checking Medicare Eligibility](https://edit.cms.gov/files/document/mln8816413-checking-medicare-eligibility.pdf)

## Billing readiness is a narrow gate

The derived readiness state is one of `match`, `mismatch`, `unverified`, `stale`, or `incomplete`.
It is advisory chart metadata today. A mismatch or missing/stale verification does not block note,
demographic, contact, or coverage saves. It is designed to fail closed only if a future billing or
export feature is added.

This repository currently performs no eligibility lookup, claim creation, EDI submission, payer
communication, remote card upload, OCR, or automatic chart mutation. Real patient data remains
prohibited.
The TUI refuses to open Medical Workspace against a remote app-server store. Actor labels are still
local caller-supplied text rather than authenticated clinician identity, so this research build does
not yet provide production-grade attribution.
