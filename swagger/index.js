import SwaggerUI from 'swagger-ui'

import 'swagger-ui/dist/swagger-ui.css'

const swaggerUrl = process.env.LITTLECI_HOST
  ? `${process.env.LITTLECI_HOST}/static.openapi.yaml`
  : (process.env.NODE_ENV === 'production' ? '/static/openapi.yaml' : 'http://localhost:8000/static/openapi.yaml')

const ui = SwaggerUI({
  url: swaggerUrl,
  dom_id: '#swagger',
})
