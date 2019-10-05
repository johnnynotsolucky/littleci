import SwaggerUI from 'swagger-ui-dist'

const SwaggerUIBundle = SwaggerUI.SwaggerUIBundle
const SwaggerUIStandalonePreset = SwaggerUI.SwaggerUIStandalonePreset

import 'swagger-ui-dist/swagger-ui.css'

const swaggerUrl = process.env.LITTLECI_HOST
  ? `${process.env.LITTLECI_HOST}/static.openapi.yaml`
  : (process.env.NODE_ENV === 'dev' ? 'http://localhost:8000/static/openapi.yaml' : '/static/openapi.yaml')

const ui = SwaggerUIBundle({
  url: swaggerUrl,
  dom_id: '#swagger',
  presets: [
    SwaggerUIBundle.presets.apis,
    SwaggerUIStandalonePreset
  ],
  layout: "StandaloneLayout"
})
