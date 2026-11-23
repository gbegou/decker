# Presenting

**Present** opens the audience window fullscreen on the projector (the first monitor that is
not the primary one) and the presenter window on the laptop screen. With a single monitor,
only the audience window opens.

![Windows during a presentation](images/windows.svg)

The presenter window shows the current slide, what the next press of a key will show (the
next build step or the next slide), the notes with adjustable text size, an elapsed-time
counter and the clock, and a strip of thumbnails to jump anywhere.

Keys work in every window, so a clicker does too:

| Key | Action |
| --- | --- |
| Right, Down, Page Down, Space, Enter, click | Next step or slide |
| Left, Up, Page Up, Backspace, right click | Previous step or slide |
| Home, End | First, last slide |
| B, `.` | Black screen on and off |
| W, `,` | White screen on and off, to write on a whiteboard under the projector |
| Escape | End the presentation |

Both windows draw the same state held by the Rust side, so they can never disagree.
