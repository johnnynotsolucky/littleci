module.exports = {
	transpileDependencies: [
		"vuetify"
	],
	publicPath: process.env.NODE_ENV === "production"
		? "/ui/"
		: "/",
	pwa: {
		workboxPluginMode:'InjectManifest',
		workboxOptions: {
			swSrc: './app/sw.js', /* Empty file. */
		}
	}
}
