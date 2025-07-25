import { collapse_and_show } from "./collapser.js";

document.addEventListener('DOMContentLoaded', async () => {
    await collapse_and_show(document.getElementById('code-table'));
});
