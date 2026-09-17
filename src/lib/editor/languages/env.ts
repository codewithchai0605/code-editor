import { type CompletionContext, snippetCompletion } from "@codemirror/autocomplete";
import { StreamLanguage, type StringStream } from "@codemirror/language";
import { tags } from "@lezer/highlight";

const COMMON_ENV_VARS = [
    ["NODE_ENV", "Defines the environment mode"],
    ["PORT", "Server listening port"],
    ["HOST", "Host binding"],
    ["DATABASE_URL", "Primary database connection string"],
    ["MONGO_URI", "MongoDB connection string"],
    ["MONGODB_URI", "MongoDB connection string"],
    ["REDIS_URL", "Redis cache connection string"],
    ["JWT_SECRET", "Secret key for JSON Web Tokens"],
    ["JWT_EXPIRES_IN", "JWT expiration time"],
    ["API_KEY", "General API Key"],
    ["SECRET_KEY", "General secret key"],
    ["NEXT_PUBLIC_API_URL", "Next.js exposed API URL"],
    ["NEXTAUTH_URL", "NextAuth base URL"],
    ["NEXTAUTH_SECRET", "NextAuth secret"],
    ["AWS_ACCESS_KEY_ID", "AWS access key"],
    ["AWS_SECRET_ACCESS_KEY", "AWS secret key"],
    ["AWS_REGION", "AWS default region"],
    ["CLOUDINARY_API_KEY", "Cloudinary API key"],
    ["CLOUDINARY_API_SECRET", "Cloudinary API secret"],
    ["CLOUDINARY_CLOUD_NAME", "Cloudinary cloud name"],
    ["STRIPE_SECRET_KEY", "Stripe secret key"],
    ["STRIPE_WEBHOOK_SECRET", "Stripe webhook secret"],
    ["GOOGLE_CLIENT_ID", "Google OAuth client id"],
    ["GOOGLE_CLIENT_SECRET", "Google OAuth client secret"],
    ["SENTRY_DSN", "Sentry data source name"],
];

const standardEnvVars = new Set(COMMON_ENV_VARS.map(([name]) => name));

const envCompletions = (context: CompletionContext) => {
    const word = context.matchBefore(/[\w$]*/);
    if (!word) return null;
    if (word.from === word.to && !context.explicit) return null;

    return {
        from: word.from,
        options: [
            ...COMMON_ENV_VARS.map(([label, info]) => ({ label, type: "constant", info })),
            { label: "production", type: "text", info: "Production environment" },
            { label: "development", type: "text", info: "Development environment" },
            { label: "test", type: "text", info: "Test environment" },
            { label: "true", type: "keyword" },
            { label: "false", type: "keyword" },
            { label: "null", type: "keyword" },
            snippetCompletion(
                'DATABASE_URL="${1:postgresql://user:password@localhost:5432/mydb}"',
                {
                    label: "db_url",
                    type: "keyword",
                    info: "Generate Postgres URL template",
                },
            ),
            snippetCompletion('REDIS_URL="${1:redis://localhost:6379}"', {
                label: "redis_url",
                type: "keyword",
                info: "Generate Redis URL template",
            }),
            snippetCompletion('${1:API_KEY}="${2:your_secret_here}"', {
                label: "secret_var",
                type: "snippet",
                info: "Generate a secret variable entry",
            }),
        ],
    };
};

export const envLanguage = StreamLanguage.define({
    startState() {
        return { afterEquals: false, inString: null };
    },
    token(stream: StringStream, state: { afterEquals: boolean; inString: string | null }) {
        if (stream.sol() && !state.inString) state.afterEquals = false;
        if (stream.eatSpace()) return null;

        if (state.inString) {
            let escaped = false;
            while (!stream.eol()) {
                const char = stream.next();
                if (char === state.inString && !escaped) {
                    state.inString = null;
                    break;
                }
                escaped = !escaped && char === "\\";
            }
            return "string";
        }

        if (stream.peek() === "#") {
            const remaining = stream.string.slice(stream.pos);
            if (/^#\s*(TODO|FIXME|BUG|NOTE|HACK):?/i.test(remaining)) {
                stream.skipToEnd();
                return "todoComment";
            }
            stream.skipToEnd();
            return "comment";
        }

        if (!state.afterEquals && stream.match(/^export\b/)) return "keyword";

        if (!state.afterEquals && stream.match(/^[A-Za-z_][A-Za-z0-9_]*/)) {
            return standardEnvVars.has(stream.current()) ? "standardVariable" : "variableName";
        }

        if (!state.afterEquals && stream.eat("=")) {
            state.afterEquals = true;
            return "operator";
        }

        if (state.afterEquals) {
            const char = stream.peek();
            if (char === '"' || char === "'") {
                state.inString = char;
                stream.next();
                return "string";
            }
            if (stream.match(/^(true|false|yes|no|on|off)\b/i)) return "bool";
            if (stream.match(/^(null|undefined)\b/i)) return "null";
            if (stream.match(/^https?:\/\/[^\s#]+/)) return "url";
            if (
                stream.match(/^\$[A-Za-z_][A-Za-z0-9_]*/) ||
                stream.match(/^\$\{[A-Za-z_][A-Za-z0-9_]*\}/)
            )
                return "envRef";
            if (stream.match(/^0x[a-fA-F0-9]+/i) || stream.match(/^-?\d*\.?\d+(e[+-]?\d+)?/i))
                return "number";
            if (stream.match(/^[^'"\s#]+/)) return "string";
        }

        stream.next();
        return null;
    },

    tokenTable: {
        comment: tags.comment,
        todoComment: tags.special(tags.comment),
        keyword: tags.keyword,
        string: tags.string,
        number: tags.number,
        operator: tags.operator,
        variableName: tags.variableName,
        standardVariable: tags.propertyName,
        bool: tags.bool,
        null: tags.null,
        url: tags.link,
        envRef: tags.special(tags.variableName),
    },
    blankLine(state) {},
    languageData: {
        // The key here must be "autocomplete" (a `CompletionSource`), which
        // is what CodeMirror's own `autocompletion()` extension (already
        // added once, globally, in setup.ts) actually reads back out of
        // language data when no `override` is configured. The previous
        // version stored the *result of calling* `autocompletion(...)` —
        // a whole separate Extension bundling its own state fields/keymap —
        // under the key "autocompletion", which nothing reads: the real
        // extension never saw `envCompletions`, so env-file autocomplete
        // was silently inert.
        autocomplete: envCompletions,
        commentTokens: { line: "#" },
    },
});
