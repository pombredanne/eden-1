from edenscmnative import diffhelpers

        line = line.rstrip(b" \r\n")
        if line.startswith(b"diff --git a/"):
                gp = patchmeta(pycompat.decodeutf8(dst))
            if line.startswith(b"--- "):
            if line.startswith(b"rename from "):
                gp.oldpath = pycompat.decodeutf8(line[12:])
            elif line.startswith(b"rename to "):
                gp.path = pycompat.decodeutf8(line[10:])
            elif line.startswith(b"copy from "):
                gp.oldpath = pycompat.decodeutf8(line[10:])
            elif line.startswith(b"copy to "):
                gp.path = pycompat.decodeutf8(line[8:])
            elif line.startswith(b"deleted file"):
            elif line.startswith(b"new file mode "):
            elif line.startswith(b"new mode "):
            elif line.startswith(b"GIT binary patch"):
        return iter(self.readline, b"")
unidesc = re.compile(b"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
                if self.lines[0].endswith(b"\r\n"):
                    self.eol = b"\r\n"
                elif self.lines[0].endswith(b"\n"):
                    self.eol = b"\n"
                        if l.endswith(b"\r\n"):
                            l = l[:-2] + b"\n"
        self.backend.setfile(fname, b"".join(lines), mode, self.copysource)
            fromfile, tofile = [pycompat.decodeutf8(f) for f in match.groups()]
            return [
                pycompat.decodeutf8(f)
                for f in self.diff_re.match(self.header[0]).groups()
            ]
        # type: () -> str
                return pycompat.decodeutf8(matched.group(1))
            "'%s'" % f for f in h.files()
                msg = messages["single"][operation] % chunk.filename()
                msg = messages["multiple"][operation] % (idx, total, chunk.filename())
        if l.startswith(b"\ "):
def parsefilename(s):
    # type: bytes -> str
    s = pycompat.decodeutf8(s)[4:].rstrip("\r\n")
    for x in iter(lr.readline, b""):
            (not context and x[0:1] == b"@")
            or (context is not False and x.startswith(b"***************"))
            or x.startswith(b"GIT binary patch")
            if x.startswith(b"GIT binary patch"):
                if context is None and x.startswith(b"***************"):
        elif x.startswith(b"diff --git a/"):
            m = gitre.match(x.rstrip(b" \r\n"))
            afile = "a/" + pycompat.decodeutf8(m.group(1))
            bfile = "b/" + pycompat.decodeutf8(m.group(2))
                file = pycompat.decodeutf8(gp.path)
                yield "file", ("a/" + file, "b/" + file, None, gp.copy())
        elif x.startswith(b"---"):
            if not l2.startswith(b"+++"):
        elif x.startswith(b"***"):
            if not l2.startswith(b"---"):
            if not l3.startswith(b"***************"):
        file = pycompat.decodeutf8(gp.path)
        yield "file", ("a/" + file, "b/" + file, None, gp.copy())
                        data = b""