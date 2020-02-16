<template>
	<v-card flat style="border-radius: 0;">
		<v-toolbar
			flat
			dark
			>
			<v-toolbar-title>
				Config
			</v-toolbar-title>
			<v-spacer />
				<v-toolbar-items>
					<v-chip dark class="align-self-center">readonly</v-chip>
				</v-toolbar-items>
		</v-toolbar>
		<v-container
			v-if="state.config"
			fluid
			class="grey lighten-4 fill-height"
			>
			<v-row>
				<v-col>
					<v-text-field
						:value="state.config.config_path"
						label="Configuration Path"
						outlined
						readonly
						></v-text-field>
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						:value="state.config.working_dir"
						label="Working Directory"
						outlined
						readonly
						></v-text-field>
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						:value="state.config.data_dir"
						label="Data Dir"
						outlined
						readonly
						></v-text-field>
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						:value="state.config.network_host"
						label="Network Host"
						outlined
						readonly
						></v-text-field>
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						:value="state.config.port"
						label="Port"
						outlined
						readonly
						></v-text-field>
				</v-col>
			</v-row>
		</v-container>
	</v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state} from '../store/state'

@Component({
	name: 'Config',
})
export default class Config extends Vue {
	state = state

	mounted() {
		this.state.loadConfig()
	}
}
</script>
