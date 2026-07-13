#!/usr/bin/env node

import { deserialize } from 'node:v8'

const encoded = process.argv[2]
if (!encoded) throw new Error('V8 payload is required')

process.stdout.write(JSON.stringify(deserialize(Buffer.from(encoded, 'base64'))))
