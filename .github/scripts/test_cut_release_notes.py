#!/usr/bin/env python3
"""What the release page is allowed to show.

The cutter is the only thing standing between the changelog's two audiences and
the release page, and when it lets engineering detail through, nobody notices
until the release is already published. Run by the build workflow's test job:

    python3 .github/scripts/test_cut_release_notes.py
"""

import unittest

from cut_release_notes import collect_bullets


class CollectBullets(unittest.TestCase):
    def test_a_wrapped_bullet_is_one_line(self):
        body = "- The version in the tray menu opens the release list, where\n  every version says what changed."
        self.assertEqual(
            collect_bullets(body),
            [
                "- The version in the tray menu opens the release list, where every "
                "version says what changed."
            ],
        )

    def test_an_indented_sub_bullet_stays_in_the_file(self):
        # Written flush against its parent, the way a nested list normally is.
        body = (
            "- Updates reach you again.\n"
            "    - Five releases sat as prereleases while the app polled stable.\n"
            "    - The promote button asked for a field the CLI does not have.\n"
            "- The settings panel is three groups instead of eleven rows.\n"
        )
        self.assertEqual(
            collect_bullets(body),
            [
                "- Updates reach you again.",
                "- The settings panel is three groups instead of eleven rows.",
            ],
        )

    def test_an_indented_paragraph_stays_in_the_file(self):
        body = (
            "- A dictation that never reached the app now says so.\n"
            "\n"
            "  When macOS refuses the keystrokes the words are transcribed and the\n"
            "  insert fails, announced in the success colour.\n"
        )
        self.assertEqual(
            collect_bullets(body), ["- A dictation that never reached the app now says so."]
        )

    def test_prose_without_bullets_yields_nothing(self):
        self.assertEqual(collect_bullets("Reworked the updater.\n"), [])


if __name__ == "__main__":
    unittest.main()
