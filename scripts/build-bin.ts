// Build standalone binary with bun. Usage: bun scripts/build-bin.ts [target] [outfile]
// ink statically imports react-devtools-core (dev only); stub it out.
const target = process.argv[2] as Bun.Build.CompileTarget | undefined;
const outfile = process.argv[3] ?? 'dist/bin/xplain';

const result = await Bun.build({
	entrypoints: ['src/cli.tsx'],
	compile: target ? {target, outfile} : {outfile},
	plugins: [
		{
			name: 'stub-devtools',
			setup(b) {
				b.onResolve({filter: /^react-devtools-core$/}, () => ({
					path: 'react-devtools-core',
					namespace: 'stub',
				}));
				b.onLoad({filter: /.*/, namespace: 'stub'}, () => ({
					contents: 'export default { connectToDevTools() {} };',
					loader: 'js',
				}));
			},
		},
	],
});
if (!result.success) {
	for (const l of result.logs) console.error(l);
	process.exit(1);
}
