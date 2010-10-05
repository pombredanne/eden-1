import test_util

import unittest

from mercurial import commands
from mercurial import ui
try:
    from mercurial import templatekw
    templatekw.keywords
except ImportError:
    templatekw = None


class CapturingUI(ui.ui):

    def __init__(self, *args, **kwds):
        super(CapturingUI, self).__init__(*args, **kwds)
        self._output = ""

    def write(self, msg, *args, **kwds):
        self._output += msg


class TestLogKeywords(test_util.TestBase):
    @test_util.requiresmodule(templatekw)
    def test_svn_keywords(self):
        defaults = {'date': None, 'rev': None, 'user': None}
        repo = self._load_fixture_and_fetch('two_revs.svndump')

        # we want one commit that isn't from Subversion
        self.commitchanges([('foo', 'foo', 'frobnicate\n')])

        ui = CapturingUI()
        commands.log(ui, repo, template='{rev}:{svnrev} ', **defaults)
        self.assertEqual(ui._output, '0:2 1:3 2: ')
        ui = CapturingUI()
        commands.log(ui, repo, template='{rev}:{svnpath} ', **defaults)
        self.assertEqual(ui._output, '0:/trunk 1:/trunk 2: ')
        ui = CapturingUI()
        commands.log(ui, repo, template='{rev}:{svnuuid} ', **defaults)
        self.assertEqual(ui._output,
                         ('0:df2126f7-00ab-4d49-b42c-7e981dde0bcf '
                          '1:df2126f7-00ab-4d49-b42c-7e981dde0bcf '
                          '2: '))


def suite():
    all = [unittest.TestLoader().loadTestsFromTestCase(TestLogKeywords),]
    return unittest.TestSuite(all)
