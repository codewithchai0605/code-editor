import { ExternalTokenizer } from "@lezer/lr";
import {
	LineComment,
	Identifier,
	FieldName,
	Type,
	EnumValue,
	AttributeName,
	BlockAttrName,
	String,
	Number,
	ArrayMarker,
} from './parser.terms.js';

const isAlpha = (ch: number) => (ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122) || ch === 95;
const isDigit = (ch: number) => ch >= 48 && ch <= 57;
const isAlphaNum = (ch: number) => isAlpha(ch) || isDigit(ch);

const atChar = 64;
const bracketL = 91;
const bracketR = 93;
const doubleQuote = 34;
const singleQuote = 39;
const slash = 47;
const newline = 10;
const backslash = 92;
const dash = 45;
const dot = 46;

function readIdent(input: any): number {
	let len = 0;
	let next = input.peek(len);
	if (!isAlpha(next)) return -1;
	while (isAlphaNum(input.peek(len))) len++;
	return len;
}

export const prismaTokens = new ExternalTokenizer(
	(input: any, stack: any) => {
		let next = input.next;

		if (next === slash && input.peek(1) === slash) {
			input.advance();
			input.advance();
			while (input.next !== -1 && input.next !== newline) input.advance();
			input.acceptToken(LineComment);
			return;
		}

		if (next === doubleQuote || next === singleQuote) {
			const quote = next;
			input.advance();
			while (input.next !== -1) {
				if (input.next === backslash) {
					input.advance();
					if (input.next !== -1) input.advance();
				} else if (input.next === quote) {
					input.advance();
					break;
				} else {
					input.advance();
				}
			}
			input.acceptToken(String);
			return;
		}

		if (next === atChar) {
			input.advance();
			if (input.next === atChar) {
				input.advance();
				const len = readIdent(input);
				if (len > 0) {
					for (let i = 0; i < len; i++) input.advance();
					input.acceptToken(BlockAttrName);
					return;
				}
			} else {
				const len = readIdent(input);
				if (len > 0) {
					for (let i = 0; i < len; i++) input.advance();
					input.acceptToken(AttributeName);
					return;
				}
			}
		}

		if (next === bracketL && input.peek(1) === bracketR) {
			input.advance();
			input.advance();
			input.acceptToken(ArrayMarker);
			return;
		}

		if (isDigit(next) || (next === dash && isDigit(input.peek(1)))) {
			input.advance();
			while (isDigit(input.next) || input.next === dot) input.advance();
			input.acceptToken(Number);
			return;
		}

		if (isAlpha(next)) {
			const len = readIdent(input);
			if (len > 0) {
				if (stack.canShift(EnumValue)) {
					for (let i = 0; i < len; i++) input.advance();
					input.acceptToken(EnumValue);
					return;
				}
				if (stack.canShift(FieldName)) {
					for (let i = 0; i < len; i++) input.advance();
					input.acceptToken(FieldName);
					return;
				}
				if (stack.canShift(Type)) {
					for (let i = 0; i < len; i++) input.advance();
					input.acceptToken(Type);
					return;
				}
				if (stack.canShift(Identifier)) {
					for (let i = 0; i < len; i++) input.advance();
					input.acceptToken(Identifier);
					return;
				}
			}
		}
	},
	{ contextual: true, fallback: true }
);
