import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import UnoCSS from 'unocss/vite'
import Icons from 'unplugin-icons/vite'

export default defineConfig({
	plugins: [UnoCSS(), sveltekit(), Icons({
      compiler: 'svelte',
    }),]
});
