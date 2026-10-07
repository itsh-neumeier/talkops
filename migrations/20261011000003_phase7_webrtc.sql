-- Phase 7: the browser softphone registers as its own device kind.
ALTER TYPE device_kind ADD VALUE IF NOT EXISTS 'browser';
