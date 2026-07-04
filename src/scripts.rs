use std::collections::HashMap;

pub struct Script {
    pub description: String,
    pub code: String,
}

pub fn find(tag: &str) -> Option<Script> {
    builtins().remove(tag)
}

pub fn list() -> Vec<(String, String)> {
    let b = builtins();
    let mut tags: Vec<_> = b.into_iter().map(|(k, v)| (k, v.description)).collect();
    tags.sort_by(|a, b| a.0.cmp(&b.0));
    tags
}

fn builtins() -> HashMap<String, Script> {
    let mut m = HashMap::new();

    m.insert("admin".into(), Script {
        description: "Admin panels and login portals. Requires URL path match + form/login markers in body.".into(),
        code: r#"function match(url, body, status, headers) {
            var u = url.toLowerCase();
            var paths = ['/admin','/login','/wp-admin','/wp-login','/dashboard','/administrator','/panel','/manage','/backend','/admin.php','/admin/login','/administrator/index.php','/user/login','/auth/login','/signin'];
            var hit = false;
            for (var i = 0; i < paths.length; i++) {
                var p = paths[i];
                if (u.indexOf(p) !== -1 && (u[u.indexOf(p) + p.length] || '') !== '-' && (u[u.indexOf(p) + p.length] || '') !== 's') { hit = true; break; }
            }
            if (!hit) return false;
            var b = body.toLowerCase();
            if (b.indexOf('/wp-admin/') !== -1 || b.indexOf('/wp-content/') !== -1 || b.indexOf('wp-login.php') !== -1) return true;
            if (b.indexOf('type="password"') !== -1 && (b.indexOf('<form') !== -1 || b.indexOf('name="pass') !== -1 || b.indexOf('id="pass') !== -1)) return true;
            if (b.indexOf('action="') !== -1 && b.indexOf('method="post"') !== -1 && b.indexOf('password') !== -1 && (b.indexOf('username') !== -1 || b.indexOf('email') !== -1)) return true;
            if ((b.indexOf('admin panel') !== -1 || b.indexOf('control panel') !== -1) && (b.indexOf('<nav') !== -1 || b.indexOf('sidebar') !== -1 || b.indexOf('menu-item') !== -1)) return true;
            if (b.indexOf('dashboard') !== -1 && (b.indexOf('logout') !== -1 || b.indexOf('profile') !== -1)) return true;
            return false;
        }"#.into(),
    });

    m.insert("env".into(), Script {
        description: "Exposed .env files. Detects KEY=VALUE patterns and checks for common environment variable names, excludes HTML/JSON/XML.".into(),
        code: r#"function match(url, body, status, headers) {
    if (body.length < 20) return false;

    const lower = body.toLowerCase();
    if (
        lower.includes("</html>") ||
        lower.includes("<html") ||
        lower.includes("<!doctype") ||
        lower.includes("<body") ||
        lower.includes("<?xml")
    ) {
        return false;
    }

    const trimmed = body.trim();
    if (
        (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
        (trimmed.startsWith("[") && trimmed.endsWith("]"))
    ) {
        return false;
    }

    const envRegex =
        /^(?:export\s+)?[A-Za-z_][A-Za-z0-9_]*\s*=\s*.*$/gm;
    const matches = body.match(envRegex) || [];
    if (matches.length < 2) return false;

    const commonVars = [
        "APP_ENV",
        "APP_KEY",
        "APP_NAME",
        "DB_HOST",
        "DB_DATABASE",
        "DB_USERNAME",
        "DB_PASSWORD",
        "DATABASE_URL",
        "SECRET_KEY",
        "JWT_SECRET",
        "API_KEY",
        "REDIS_HOST",
        "MAIL_HOST",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "NODE_ENV",
        "PORT"
    ];
    const lowerBody = body.toLowerCase();
    const hasCommonVar = commonVars.some(v => lowerBody.includes(v.toLowerCase()));

    if (!hasCommonVar && matches.length < 3) {
        return false;
    }

    return true;
}"#.into(),
    });

    m.insert("sqli".into(), Script {
        description: "SQL injection error disclosure. Matches actual DB error messages from MySQL, PostgreSQL, MSSQL, Oracle.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body.toLowerCase();
            if (b.indexOf('sql syntax') !== -1 && (b.indexOf('near') !== -1 || b.indexOf('at line') !== -1 || b.indexOf('error in your sql') !== -1)) return true;
            if (b.indexOf('unclosed quotation mark') !== -1 || b.indexOf('quoted string not properly terminated') !== -1 || b.indexOf('ora-0') !== -1) return true;
            if (b.indexOf('postgresql') !== -1 && b.indexOf('error:') !== -1 && b.indexOf('sql state:') !== -1) return true;
            if ((b.indexOf('microsoft ole db') !== -1 || b.indexOf('sql server') !== -1 || b.indexOf('odbc driver') !== -1) && b.indexOf('error') !== -1) return true;
            if (b.indexOf('sqlite3::syntaxerror') !== -1 || b.indexOf('sqlite_error') !== -1 || b.indexOf('pg_query():') !== -1 || b.indexOf('mysql_fetch') !== -1 && b.indexOf('error') !== -1) return true;
            if (b.indexOf('warning: mysql') !== -1 && b.indexOf('supplied argument') !== -1) return true;
            if (b.indexOf('column') !== -1 && b.indexOf('does not exist') !== -1 && b.indexOf('relation') === -1) return false;
            return false;
        }"#.into(),
    });

    m.insert("stacktrace".into(), Script {
        description: "Stack traces and verbose debug output. Python/PHP/Node/Java/Ruby stack traces and fatal errors.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body;
            var lines = b.split('\n');
            var traceLines = 0;
            var hasHeader = false;
            if (b.indexOf('Traceback (most recent call last):') !== -1) hasHeader = true;
            if (b.indexOf('Exception in thread') !== -1 && b.indexOf(' at ') !== -1) hasHeader = true;
            if ((b.indexOf('Fatal error:') !== -1 || b.indexOf('Parse error:') !== -1 || b.indexOf('Warning:') !== -1) && b.indexOf(' on line ') !== -1 && b.indexOf(' in /') !== -1) return true;
            if ((b.indexOf('Error:') !== -1 || b.indexOf('TypeError:') !== -1 || b.indexOf('ReferenceError:') !== -1 || b.indexOf('SyntaxError:') !== -1 || b.indexOf('RangeError:') !== -1) && b.indexOf(' at ') !== -1 && (b.indexOf('.js:') !== -1 || b.indexOf('.ts:') !== -1)) return true;
            if (b.indexOf('called in /') !== -1 && b.indexOf('on line') !== -1) return true;
            for (var i = 0; i < lines.length; i++) {
                var l = lines[i].trim();
                if (l.indexOf('File "') === 0 && l.indexOf('", line ') !== -1) traceLines++;
                if (l.indexOf('at ') === 0 && l.indexOf('(') !== -1 && l.indexOf(':') !== -1 && l.indexOf(')') !== -1) traceLines++;
                if ((l.indexOf('.java:') !== -1 || l.indexOf('.py:') !== -1 || l.indexOf('.rb:') !== -1 || l.indexOf('.php:') !== -1 || l.indexOf('.go:') !== -1) && (l.indexOf(' at ') !== -1 || l.indexOf('\t') === 0)) traceLines++;
                if (l.indexOf('#') === 0 && l.indexOf(' ') !== -1 && l.indexOf(':') !== -1) traceLines++;
            }
            if (hasHeader && traceLines >= 2) return true;
            if (traceLines >= 4) return true;
            return false;
        }"#.into(),
    });

    m.insert("dirlist".into(), Script {
        description: "Open directory listing pages. Apache/Nginx/IIS auto-index with file tables.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body.toLowerCase();
            if (b.indexOf('index of /') !== -1 && b.indexOf('parent directory') !== -1) return true;
            if (b.indexOf('directory listing for') !== -1 && b.indexOf('last modified') !== -1 && b.indexOf('<table') !== -1) return true;
            if (b.indexOf('to parent directory</a>') !== -1 && b.indexOf('last modified') !== -1 && b.indexOf('size') !== -1) return true;
            if (b.indexOf('[To Parent Directory]</a>') !== -1 && b.indexOf('</a>') !== -1 && b.indexOf('&lt;dir&gt;') !== -1) return true;
            if (b.indexOf('" alt="[DIR]"') !== -1 && b.indexOf('" alt="[   ]"') === -1 && b.indexOf('" alt="[DIR]"') !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("upload".into(), Script {
        description: "File upload forms and endpoints. Detects multipart forms and upload paths.".into(),
        code: r#"function match(url, body, status, headers) {
            var u = url.toLowerCase();
            var uploadPaths = ['upload', 'uploader', 'file-upload', 'fileupload', 'attachment', 'import', 'bulk'];
            var pathMatch = false;
            for (var i = 0; i < uploadPaths.length; i++) { if (u.indexOf('/' + uploadPaths[i]) !== -1 || u.indexOf('=' + uploadPaths[i]) !== -1 || u.indexOf('/' + uploadPaths[i] + '.') !== -1) { pathMatch = true; break; } }
            var b = body.toLowerCase();
            if (b.indexOf('enctype="multipart/form-data"') !== -1 && b.indexOf('type="file"') !== -1) return true;
            if (pathMatch && b.indexOf('type="file"') !== -1 && b.indexOf('<input') !== -1) return true;
            if (b.indexOf('formdata') !== -1 && b.indexOf('.append(') !== -1 && b.indexOf("'file'") !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("apispec".into(), Script {
        description: "API documentation and specs. Swagger, ReDoc, GraphQL playgrounds, OpenAPI schemas.".into(),
        code: r#"function match(url, body, status, headers) {
            var u = url.toLowerCase();
            var b = body.toLowerCase();
            if (b.indexOf('swagger-ui') !== -1 && b.indexOf('swagger.json') !== -1) return true;
            if (u.indexOf('/swagger') !== -1 && (b.indexOf('"openapi"') !== -1 || b.indexOf('"swagger"') !== -1)) return true;
            if (b.indexOf('<redoc') !== -1 || b.indexOf('redoc standalone') !== -1) return true;
            if (b.indexOf('graphiql') !== -1 && (b.indexOf('graphql') !== -1 || b.indexOf('query ') !== -1)) return true;
            if (u.indexOf('/graphql') !== -1 && b.indexOf('"data"') !== -1 && b.indexOf('"errors"') !== -1) return true;
            if ((u.indexOf('/api-docs') !== -1 || u.indexOf('/api/docs') !== -1 || u.indexOf('/openapi') !== -1) && (b.indexOf('"paths"') !== -1 || b.indexOf('"info"') !== -1) && b.indexOf('"title"') !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("secrets".into(), Script {
        description: "Exposed credentials, API keys, and tokens in responses. AWS/GCP keys, JWT tokens, private keys.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body;
            var c = 0;
            if (b.indexOf('-----BEGIN RSA PRIVATE KEY-----') !== -1 || b.indexOf('-----BEGIN PRIVATE KEY-----') !== -1 || b.indexOf('-----BEGIN EC PRIVATE KEY-----') !== -1) return true;
            if (b.indexOf('-----BEGIN OPENSSH PRIVATE KEY-----') !== -1 || b.indexOf('-----BEGIN DSA PRIVATE KEY-----') !== -1) return true;
            var akia = b.indexOf('AKIA');
            if (akia !== -1 && b.length > akia + 20) { var rest = b.substring(akia + 4, akia + 20); if (/^[A-Z0-9]{16}$/.test(rest)) return true; }
            if (b.indexOf('ya29.') !== -1 && b.indexOf('googleapis') !== -1) return true;
            var jwt = b.indexOf('eyJ');
            if (jwt !== -1 && b.length > jwt + 50) { var seg = b.substring(jwt, jwt + 50); if (seg.indexOf('.') !== -1 && seg.split('.').length >= 3) return true; }
            if (b.indexOf('hooks.slack.com/services/') !== -1) return true;
            if (b.indexOf('ghp_') !== -1 || b.indexOf('gho_') !== -1 || b.indexOf('ghu_') !== -1 || b.indexOf('ghs_') !== -1 || b.indexOf('ghr_') !== -1) return true;
            if (b.indexOf('mongodb+srv://') !== -1 || b.indexOf('mysql://') !== -1 || b.indexOf('postgresql://') !== -1 || b.indexOf('postgres://') !== -1 || b.indexOf('redis://') !== -1) return true;
            if (b.indexOf('\"password\"') !== -1 || b.indexOf('\"secret\"') !== -1 || b.indexOf('\"api_key\"') !== -1) c++;
            if (b.indexOf('password=') !== -1 || b.indexOf('passwd=') !== -1 || b.indexOf('pwd=') !== -1) c++;
            if (c >= 2) return true;
            return false;
        }"#.into(),
    });

    m.insert("backups".into(), Script {
        description: "Exposed backup archives and database dumps. .sql, .bak, .tar.gz, .zip files.".into(),
        code: r#"function match(url, body, status, headers) {
            var u = url.toLowerCase();
            var exts = ['.sql', '.sql.gz', '.sql.bz2', '.dump', '.bak', '.backup', '.tar', '.tar.gz', '.tar.bz2', '.zip', '.7z', '.rar', '.gz', '.bz2', '.tgz', '.db', '.sqlite', '.sqlite3', '.mdb'];
            for (var i = 0; i < exts.length; i++) {
                var e = exts[i];
                if (u.indexOf(e) !== -1 && u.indexOf(e) === u.length - e.length) return true;
                if (u.indexOf('/' + e.substring(1)) !== -1 && headers.toLowerCase().indexOf('content-type: text/html') === -1) return true;
            }
            var b = body.toLowerCase();
            if ((b.indexOf('index of /') !== -1 || b.indexOf('directory listing') !== -1) && b.indexOf('parent directory') !== -1) {
                for (var i = 0; i < exts.length; i++) { if (b.indexOf(exts[i]) !== -1) return true; }
            }
            var backupTerms = ['backup', 'dump', 'export', 'database', 'databases', 'backups', 'archive', 'snapshot'];
            var urlTerm = false;
            for (var i = 0; i < backupTerms.length; i++) { if (u.indexOf(backupTerms[i]) !== -1) { urlTerm = true; break; } }
            if (urlTerm && body.length > 100 && (body.indexOf('mysql') !== -1 || body.indexOf('postgresql') !== -1 || body.indexOf('sqlite') !== -1 || body.indexOf('create table') !== -1 || body.indexOf('insert into') !== -1)) return true;
            return false;
        }"#.into(),
    });

    m.insert("phpinfo".into(), Script {
        description: "PHP info/configuration pages exposing server details. Requires version + config table markers.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body.toLowerCase();
            if (b.indexOf('php version') === -1) return false;
            if (b.indexOf('</title>') === -1 && b.indexOf('php') === -1) return false;
            if (b.indexOf('system </td>') !== -1 || b.indexOf('build date </td>') !== -1 || b.indexOf('server api </td>') !== -1 || b.indexOf('virtual directory support </td>') !== -1 || b.indexOf('configuration file (php.ini) path </td>') !== -1 || b.indexOf('loaded configuration file </td>') !== -1) return true;
            if (b.indexOf('<h1 class="p">php version') !== -1 || b.indexOf('<h2>php license</h2>') !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("git".into(), Script {
        description: "Exposed .git repositories. Detects git objects, refs, config, and pack files.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body;
            if (b.indexOf('[core]') !== -1 && b.indexOf('repositoryformatversion') !== -1) return true;
            if (b.indexOf('refs/heads/') !== -1 || b.indexOf('refs/tags/') !== -1 || b.indexOf('refs/remotes/') !== -1) return true;
            if (b.indexOf('# pack-refs with:') !== -1) return true;
            if (b.indexOf('x') === 0 && b.indexOf('blob') !== -1 && b.indexOf('\0') !== -1) return true;
            var u = url.toLowerCase();
            if (u.indexOf('.git/') !== -1 && (u.indexOf('/objects/') !== -1 || u.indexOf('/refs/') !== -1 || u.indexOf('/logs/') !== -1)) return true;
            if (u.indexOf('.git/config') !== -1 && b.indexOf('[remote ') !== -1) return true;
            if (u.indexOf('.git/head') !== -1 && (b.indexOf('ref: refs/') !== -1 || b.length === 41)) return true;
            return false;
        }"#.into(),
    });

    m.insert("errorpages".into(), Script {
        description: "Error pages leaking server version, paths, and configuration details.".into(),
        code: r#"function match(url, body, status, headers) {
            if (status > 0 && (status < 400 || status >= 600)) return false;
            var b = body.toLowerCase();
            if (b.indexOf('server at') !== -1 && b.indexOf('port ') !== -1 && (b.indexOf('apache') !== -1 || b.indexOf('nginx') !== -1 || b.indexOf('microsoft-iis') !== -1)) return true;
            if (b.indexOf('microsoft .net framework version:') !== -1 || b.indexOf('asp.net version:') !== -1) return true;
            if (b.indexOf('tomcat/') !== -1 && b.indexOf('jsp') !== -1 && b.indexOf('exception report') !== -1) return true;
            if ((b.indexOf('description:') !== -1 && b.indexOf(' an unhandled exception occurred') !== -1) || (b.indexOf('the server encountered an internal error') !== -1 && b.indexOf('jboss') !== -1)) return true;
            if (status >= 500 && (b.indexOf('exception') !== -1 || b.indexOf('stack trace') !== -1 || b.indexOf('stacktrace') !== -1)) return true;
            if (status > 0 && status < 600 && (b.indexOf('/var/www') !== -1 || b.indexOf('/home/') !== -1 || b.indexOf('/usr/share') !== -1 || b.indexOf('c:\\inetpub') !== -1 || b.indexOf('c:\\xampp') !== -1) && b.indexOf('error') !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("docker".into(), Script {
        description: "Exposed Docker APIs, registries, and misconfigured containers.".into(),
        code: r#"function match(url, body, status, headers) {
            var u = url.toLowerCase();
            var b = body.toLowerCase();
            if (u.indexOf('/containers/json') !== -1 && b.indexOf('\"Id\"') !== -1 && b.indexOf('\"Image\"') !== -1 && b.indexOf('\"Command\"') !== -1) return true;
            if (u.indexOf('/v') !== -1 && (u.indexOf('/version') !== -1 || u.indexOf('/info') !== -1) && b.indexOf('\"ApiVersion\"') !== -1) return true;
            if (u.indexOf('/v2/_catalog') !== -1 && b.indexOf('\"repositories\"') !== -1) return true;
            if (b.indexOf('dockerfile') !== -1 && b.indexOf('from ') !== -1 && b.indexOf('run ') !== -1 && b.indexOf('cmd ') !== -1 && b.indexOf('copy ') !== -1) return true;
            if (headers.toLowerCase().indexOf('docker-distribution-api-version') !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("cors".into(), Script {
        description: "CORS misconfigurations exposing internal services. Detects overly permissive Access-Control headers.".into(),
        code: r#"function match(url, body, status, headers) {
            var h = headers.toLowerCase();
            var acao = h.indexOf('access-control-allow-origin:');
            if (acao === -1 && h.indexOf('access-control-allow-credentials: true') === -1) return false;
            if (h.indexOf('access-control-allow-origin: *') !== -1 && h.indexOf('access-control-allow-credentials: true') !== -1) return true;
            if (h.indexOf('access-control-allow-origin: null') !== -1 || h.indexOf('access-control-allow-methods:') !== -1 && h.indexOf('PUT') !== -1 && h.indexOf('DELETE') !== -1) return true;
            var b = body.toLowerCase();
            if ((b.indexOf('\"access_key_id\"') !== -1 || b.indexOf('\"secret_access_key\"') !== -1 || b.indexOf('\"session_token\"') !== -1) && h.indexOf('access-control-allow-origin') !== -1) return true;
            return false;
        }"#.into(),
    });

    m.insert("wafbypass".into(), Script {
        description: "WAF bypass indicators: pages that might indicate a bypassed/absent WAF. Server headers and verbose 200s on attack paths.".into(),
        code: r#"function match(url, body, status, headers) {
            var u = url.toLowerCase();
            var attackPaths = ["'.concat('", "or '1'='1", "/etc/passwd", "../windows/win.ini", "<script>alert(1)</script>", "union select", "sleep(", "/bin/cat /etc/passwd", "cmd.exe", "whoami", "<?php", "<?="];
            var hasAttack = false;
            for (var i = 0; i < attackPaths.length; i++) { if (u.indexOf(attackPaths[i]) !== -1) { hasAttack = true; break; } }
            if (!hasAttack) return false;
            if (status !== 200) return true;
            var h = headers.toLowerCase();
            var b = body.toLowerCase();
            if (h.indexOf('server:') !== -1 && (h.indexOf('apache/2') !== -1 || h.indexOf('nginx/1') !== -1 || h.indexOf('iis/') !== -1 || h.indexOf('tomcat') !== -1)) {
                if (b.indexOf('root:') !== -1 || b.indexOf('uid=0') !== -1 || b.indexOf('[extensions]') !== -1 || b.indexOf('bit app path') !== -1) return true;
                if (b.indexOf('for 64-bit windows') !== -1 || b.indexOf('microsoft windows [version') !== -1) return true;
            }
            return false;
        }"#.into(),
    });

    m.insert("cdnbackends".into(), Script {
        description: "Origin/backend IP leaks through CDN misconfigurations. Detects internal hostnames in responses.".into(),
        code: r#"function match(url, body, status, headers) {
            var b = body;
            var internal = ['10.', '172.16.', '172.17.', '172.18.', '172.19.', '172.20.', '172.21.', '172.22.', '172.23.', '172.24.', '172.25.', '172.26.', '172.27.', '172.28.', '172.29.', '172.30.', '172.31.', '192.168.', '127.0.0.1', 'localhost', 'internal.', 'staging.', 'dev.', 'preprod.', 'qa.', 'uat.', 'test.', 'beta.'];
            var h = headers.toLowerCase();
            for (var i = 0; i < internal.length; i++) {
                var ip = internal[i];
                if (h.indexOf(ip) !== -1) return true;
                if (h.indexOf('host:') !== -1 && h.indexOf(ip) !== -1) return true;
            }
            for (var i = 0; i < internal.length; i++) {
                var ip = internal[i];
                if (b.indexOf(ip) !== -1 && (b.indexOf(ip + '.') !== -1 || b.indexOf(ip + ':') !== -1)) return true;
            }
            if (h.indexOf('x-amz-cf-') !== -1 && (b.indexOf('10.') !== -1 || b.indexOf('192.168') !== -1 || b.indexOf('internal') !== -1)) return true;
            if (h.indexOf('cf-connecting-ip') !== -1 && h.indexOf('cf-ray') !== -1 && (b.indexOf('ec2-') !== -1 || b.indexOf('compute.amazonaws.com') !== -1)) return true;
            return false;
        }"#.into(),
    });

    m
}
