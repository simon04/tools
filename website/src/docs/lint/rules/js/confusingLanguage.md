---
title: Lint Rule js/confusingLanguage
layout: layouts/page.njk
description: MISSING DOCUMENTATION
eleventyNavigation: {
	key: lint-rules/js/confusingLanguage,
	parent: lint-rules,
	title: js/confusingLanguage
}
---

# js/confusingLanguage

MISSING DOCUMENTATION

<!-- EVERYTHING BELOW IS AUTOGENERATED. SEE SCRIPTS FOLDER FOR UPDATE SCRIPTS -->


## Examples
## Invalid
```typescript
//	the	blacklist
```
```typescript
/*	the
blacklist	*/
```
```typescript
blacklist;
```
```typescript
BLACKLIST;
```
```typescript
someBlacklist;
```
```typescript
SOME_BLACKLIST;
```
## Valid