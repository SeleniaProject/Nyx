{{- define "nyx.name" -}}
nyx
{{- end -}}

{{- define "nyx.fullname" -}}
{{ include "nyx.name" . }}
{{- end -}} 

{{- define "nyx.labels" -}}
app.kubernetes.io/name: {{ include "nyx.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | default .Chart.Version }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "nyx.selectorLabels" -}}
app.kubernetes.io/name: {{ include "nyx.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}