# Shared front-end

The presentation windows (audience and presenter), the slide view with its media playback,
the pointer and the show protocol, used by both `deck-app` (the editor) and `deck-viewer`.
Each app references this folder through the `$shared` Vite alias; it is not an npm package,
so each app keeps its own `npm install`.
