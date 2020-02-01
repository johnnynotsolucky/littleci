<template>
	<v-card
		flat
		style="border-radius: 0;"
	>
		<v-toolbar
			flat
			dark
		>
			<v-toolbar-title>
				Update Your Profile
			</v-toolbar-title>
			<v-spacer />
			<v-toolbar-items>
				<v-btn @click="onSave">
					Save
					<v-icon right small>fas fa-save</v-icon>
				</v-btn>
			</v-toolbar-items>
		</v-toolbar>
		<v-container
			fluid
			class="grey lighten-4 fill-height"
		>
			<v-row>
				<v-col>
					<v-text-field
						:value="state.user.username"
						label="Username"
						readonly
						outlined
					></v-text-field>
				</v-col>
			</v-row>
			<v-row>
				<v-col>
					<v-text-field
						v-model="password"
						label="Password"
						outlined
						type="password"
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
	name: 'UpdateRepository',
})
export default class UpdateRepository extends Vue {
	state = state

	password = ''

	async onSave() {
		await this.state.setUserPassword(this.password)
	}
}
</script>


